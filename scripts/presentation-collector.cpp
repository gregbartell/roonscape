// Optional LD_PRELOAD collector for the Qt xcb / Mesa DRI3 Present path.
// The render thread copies fixed-size requests into a bounded SPSC queue.
// Only the worker connects to X, selects completion events, and writes evidence.
#include <array>
#include <atomic>
#include <chrono>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <dlfcn.h>
#include <sys/uio.h>
#include <thread>
#include <unordered_set>
#include <xcb/present.h>
#include <xcb/xcb.h>
#include <xcb/xcbext.h>

namespace {
constexpr size_t capacity = 4096;
constexpr uint64_t maximum = 131072;
uint64_t micros() {
    return std::chrono::duration_cast<std::chrono::microseconds>(
               std::chrono::steady_clock::now().time_since_epoch())
        .count();
}
struct Submission {
    uint64_t frame, scene, time, target;
    uint32_t window, serial, options;
};
using Identity = uint64_t (*)();
struct Collector {
    std::array<Submission, capacity> queue{};
    std::atomic<uint64_t> head{0}, tail{0}, lost{0}, overhead{0};
    std::atomic<bool> stopping{false};
    Identity frame, scene;
    const char *file;
    std::thread worker;
    explicit Collector(const char *destination)
        : frame(reinterpret_cast<Identity>(dlsym(RTLD_DEFAULT, "roonscape_submission_frame"))),
          scene(reinterpret_cast<Identity>(dlsym(RTLD_DEFAULT, "roonscape_submission_scene"))),
          file(destination), worker([this] { consume(); }) {}
    void enqueue(const Submission &entry) {
        const auto begin = micros();
        const auto next = head.load(std::memory_order_relaxed);
        if (next - tail.load(std::memory_order_acquire) >= capacity || next >= maximum) {
            lost.fetch_add(1, std::memory_order_relaxed);
        } else {
            queue[next % capacity] = entry;
            head.store(next + 1, std::memory_order_release);
        }
        overhead.fetch_add(micros() - begin, std::memory_order_relaxed);
    }
    void consume() {
        // fopen can block (e.g. FIFO); it never runs in a submission callback.
        FILE *out = std::strcmp(file, "-") == 0 ? stdout : std::fopen(file, "wx");
        if (!out) {
            lost++;
            return;
        }
        std::fprintf(out,
                     "{\"event\":\"start\",\"version\":1,\"clock\":\"CLOCK_MONOTONIC\","
                     "\"capacity\":4096,\"maximumRecords\":131072,\"identitySupported\":%s}\n",
                     frame && scene ? "true" : "false");
        int screen;
        auto *connection = xcb_connect(nullptr, &screen);
        const auto *extension = xcb_get_extension_data(connection, &xcb_present_id);
        bool supported = !xcb_connection_has_error(connection) && extension && extension->present;
        std::unordered_set<uint32_t> windows;
        uint64_t records = 0;
        auto completions = [&] {
            while (auto *event = xcb_poll_for_event(connection)) {
                if (event->response_type == XCB_GE_GENERIC) {
                    auto *complete = reinterpret_cast<xcb_present_complete_notify_event_t *>(event);
                    if (complete->extension == extension->major_opcode &&
                        complete->event_type == XCB_PRESENT_COMPLETE_NOTIFY &&
                        complete->kind == XCB_PRESENT_COMPLETE_KIND_PIXMAP) {
                        if (++records > maximum)
                            lost++;
                        else
                            std::fprintf(
                                out,
                                "{\"event\":\"completion\",\"window\":%u,\"serial\":%u,\"mode\":%u,"
                                "\"ust\":%llu,\"msc\":%llu,\"observedMicros\":%llu}\n",
                                complete->window, complete->serial, complete->mode,
                                (unsigned long long)complete->ust,
                                (unsigned long long)complete->msc, (unsigned long long)micros());
                    }
                } else if (event->response_type == 0)
                    lost++;
                std::free(event);
            }
        };
        while (!stopping.load(std::memory_order_acquire) || tail.load() < head.load()) {
            auto next = tail.load(std::memory_order_relaxed);
            const auto end = head.load(std::memory_order_acquire);
            while (next < end) {
                const auto entry = queue[next % capacity];
                tail.store(++next, std::memory_order_release);
                if (supported && windows.insert(entry.window).second) {
                    auto cookie = xcb_present_select_input_checked(
                        connection, xcb_generate_id(connection), entry.window,
                        XCB_PRESENT_EVENT_MASK_COMPLETE_NOTIFY);
                    auto *error = xcb_request_check(connection, cookie);
                    if (error) {
                        supported = false;
                        lost++;
                        std::free(error);
                    }
                    if (++records <= maximum)
                        std::fprintf(
                            out,
                            "{\"event\":\"subscribed\",\"window\":%u,\"observedMicros\":%llu}\n",
                            entry.window, (unsigned long long)micros());
                    else
                        lost++;
                }
                if (++records > maximum)
                    lost++;
                else
                    std::fprintf(out,
                                 "{\"event\":\"submission\",\"frame\":%llu,\"scene\":%llu,"
                                 "\"observedMicros\":%llu,\"window\":%u,\"serial\":%u,"
                                 "\"targetMsc\":%llu,\"options\":%u}\n",
                                 (unsigned long long)entry.frame, (unsigned long long)entry.scene,
                                 (unsigned long long)entry.time, entry.window, entry.serial,
                                 (unsigned long long)entry.target, entry.options);
            }
            if (supported)
                completions();
            std::fflush(out);
            if (xcb_connection_has_error(connection)) {
                supported = false;
                lost++;
                break;
            }
            std::this_thread::sleep_for(std::chrono::milliseconds(1));
        }
        if (supported)
            completions();
        const auto dropped = lost.load();
        std::fprintf(out,
                     "{\"event\":\"end\",\"complete\":%s,\"lost\":%llu,\"records\":%llu,"
                     "\"enqueueMicros\":%llu,\"submissions\":%llu,\"observedMicros\":%llu}\n",
                     supported && !dropped ? "true" : "false", (unsigned long long)dropped,
                     (unsigned long long)records, (unsigned long long)overhead.load(),
                     (unsigned long long)head.load(), (unsigned long long)micros());
        std::fclose(out);
        xcb_disconnect(connection);
    }
};
Collector *collector = nullptr;
using Send = unsigned int (*)(xcb_connection_t *, int, iovec *, const xcb_protocol_request_t *);
using Send64 = uint64_t (*)(xcb_connection_t *, int, iovec *, const xcb_protocol_request_t *);
using SendFds = unsigned int (*)(xcb_connection_t *, int, iovec *, const xcb_protocol_request_t *,
                                 unsigned int, int *);
using SendFds64 = uint64_t (*)(xcb_connection_t *, int, iovec *, const xcb_protocol_request_t *,
                               unsigned int, int *);
Send sendRequest;
Send64 sendRequest64;
SendFds sendFds;
SendFds64 sendFds64;
thread_local bool forwarding = false;
void observe(iovec *vector, const xcb_protocol_request_t *request) {
    if (!collector || !collector->frame || !collector->scene || !request->ext ||
        std::strcmp(request->ext->name, "Present") || forwarding)
        return;
    const auto frame = collector->frame(), scene = collector->scene();
    if (!frame || !scene)
        return; // Other threads/windows cannot borrow a global frame identity.
    if (request->opcode == XCB_PRESENT_PIXMAP) {
        if (vector[0].iov_len < sizeof(xcb_present_pixmap_request_t)) {
            collector->lost++;
            return;
        }
        xcb_present_pixmap_request_t pixmap;
        std::memcpy(&pixmap, vector[0].iov_base, sizeof(pixmap));
        collector->enqueue({frame, scene, micros(), pixmap.target_msc, pixmap.window, pixmap.serial,
                            pixmap.options});
    } else if (request->opcode == 5) {
        // Present 1.4 PixmapSynced wire request: identity at 4/12, options at
        // 56, target MSC at 64. Decoding does not require a newer libxcb ABI.
        if (vector[0].iov_len < 88) {
            collector->lost++;
            return;
        }
        const auto *bytes = static_cast<const unsigned char *>(vector[0].iov_base);
        Submission entry{frame, scene, micros(), 0, 0, 0, 0};
        std::memcpy(&entry.window, bytes + 4, 4);
        std::memcpy(&entry.serial, bytes + 12, 4);
        std::memcpy(&entry.options, bytes + 56, 4);
        std::memcpy(&entry.target, bytes + 64, 8);
        collector->enqueue(entry);
    }
}
struct Forwarding {
    bool previous = forwarding;
    Forwarding() { forwarding = true; }
    ~Forwarding() { forwarding = previous; }
};
__attribute__((constructor)) void initialize() {
    sendRequest = reinterpret_cast<Send>(dlsym(RTLD_NEXT, "xcb_send_request"));
    sendRequest64 = reinterpret_cast<Send64>(dlsym(RTLD_NEXT, "xcb_send_request64"));
    sendFds = reinterpret_cast<SendFds>(dlsym(RTLD_NEXT, "xcb_send_request_with_fds"));
    sendFds64 = reinterpret_cast<SendFds64>(dlsym(RTLD_NEXT, "xcb_send_request_with_fds64"));
    if (const auto *file = std::getenv("ROONSCAPE_PRESENT_EVIDENCE"))
        collector = new Collector(file);
}
__attribute__((destructor)) void finish() {
    if (collector) {
        collector->stopping.store(true, std::memory_order_release);
        collector->worker.join();
        // Keep state alive until loader teardown finishes; later hooks cannot race deletion.
    }
}
} // namespace
extern "C" unsigned int xcb_send_request(xcb_connection_t *c, int flags, iovec *v,
                                         const xcb_protocol_request_t *r) {
    observe(v, r);
    Forwarding guard;
    return sendRequest(c, flags, v, r);
}
extern "C" uint64_t xcb_send_request64(xcb_connection_t *c, int flags, iovec *v,
                                       const xcb_protocol_request_t *r) {
    observe(v, r);
    Forwarding guard;
    return sendRequest64(c, flags, v, r);
}
extern "C" unsigned int xcb_send_request_with_fds(xcb_connection_t *c, int flags, iovec *v,
                                                  const xcb_protocol_request_t *r, unsigned int n,
                                                  int *fds) {
    observe(v, r);
    Forwarding guard;
    return sendFds(c, flags, v, r, n, fds);
}
extern "C" uint64_t xcb_send_request_with_fds64(xcb_connection_t *c, int flags, iovec *v,
                                                const xcb_protocol_request_t *r, unsigned int n,
                                                int *fds) {
    observe(v, r);
    Forwarding guard;
    return sendFds64(c, flags, v, r, n, fds);
}

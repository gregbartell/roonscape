// External-process fixture exercises the installed collector against a private X server.
#include <chrono>
#include <cstdint>
#include <cstdio>
#include <cstdlib>
#include <fcntl.h>
#include <sys/socket.h>
#include <thread>
#include <unistd.h>
#include <xcb/present.h>
#include <xcb/xcb.h>

thread_local uint64_t frame = 0;
extern "C" uint64_t roonscape_submission_frame() { return frame; }
extern "C" uint64_t roonscape_submission_scene() { return frame; }
int main(int argc, char **argv) {
    if (argc != 3)
        return 2;
    const int count = std::atoi(argv[1]);
    // Bound the test consumer transport so a paused reader deterministically stalls.
    int bytes = 4096;
    setsockopt(STDOUT_FILENO, SOL_SOCKET, SO_SNDBUF, &bytes, sizeof(bytes));
    fcntl(STDOUT_FILENO, F_SETPIPE_SZ, bytes);
    // Tests control the producer through a FIFO, not scheduler delays.
    const int control = open(argv[2], O_RDWR);
    if (control < 0)
        return 3;
    auto *connection = xcb_connect(nullptr, nullptr);
    if (xcb_connection_has_error(connection))
        return 4;
    const auto *screen = xcb_setup_roots_iterator(xcb_get_setup(connection)).data;
    const auto window = xcb_generate_id(connection), pixmap = xcb_generate_id(connection);
    xcb_create_window(connection, screen->root_depth, window, screen->root, 0, 0, 64, 64, 0,
                      XCB_WINDOW_CLASS_INPUT_OUTPUT, screen->root_visual, 0, nullptr);
    xcb_map_window(connection, window);
    xcb_create_pixmap(connection, screen->root_depth, pixmap, window, 64, 64);
    auto *reply = xcb_get_input_focus_reply(connection, xcb_get_input_focus(connection), nullptr);
    std::free(reply);
    std::fprintf(stderr, "ready\n");
    std::fflush(stderr);
    char command;
    while (read(control, &command, 1) == 1) {
        if (command == 'F')
            break;
        const int batch = command == 'S' ? 1 : count;
        for (int i = 0; i < batch; ++i) {
            ++frame;
            xcb_present_pixmap(connection, window, pixmap, frame, 0, 0, 0, 0, 0, 0, 0,
                               XCB_PRESENT_OPTION_COPY, 0, 0, 0, 0, nullptr);
        }
        xcb_flush(connection);
        std::fprintf(stderr, "submitted:%llu\n", (unsigned long long)frame);
        std::fflush(stderr);
    }
    // Keep the connection/window alive until the preload destructor drains.
    close(control);
    return 0;
}

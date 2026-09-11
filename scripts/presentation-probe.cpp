// Read-only physical-path capability probe. No modes, power settings, or windows change.
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <memory>
#include <stdexcept>
#include <string>
#include <vector>
#include <xcb/randr.h>
#include <xcb/xcb.h>

template <class T> using Reply = std::unique_ptr<T, decltype(&std::free)>;
template <class T> Reply<T> owned(T *value) {
    if (!value)
        throw std::runtime_error("X11 query unavailable");
    return {value, &std::free};
}
int main(int argc, char **argv) {
    try {
        if (argc != 4)
            throw std::runtime_error("probe requires OUTPUT WIDTH HEIGHT");
        int number;
        auto *connection = xcb_connect(nullptr, &number);
        if (xcb_connection_has_error(connection))
            throw std::runtime_error("selected X11 display is unavailable");
        auto screens = xcb_setup_roots_iterator(xcb_get_setup(connection));
        for (int i = 0; i < number; ++i)
            xcb_screen_next(&screens);
        const auto root = screens.data->root;
        for (const char *name : {"Present", "DRI3", "RANDR"}) {
            auto extension = owned(xcb_query_extension_reply(
                connection, xcb_query_extension(connection, std::strlen(name), name), nullptr));
            if (!extension->present)
                throw std::runtime_error(std::string("unsupported X11 extension: ") + name);
        }
        const auto selection = std::string("_NET_WM_CM_S") + std::to_string(number);
        auto atom = owned(xcb_intern_atom_reply(
            connection, xcb_intern_atom(connection, true, selection.size(), selection.c_str()),
            nullptr));
        if (atom->atom) {
            auto owner = owned(xcb_get_selection_owner_reply(
                connection, xcb_get_selection_owner(connection, atom->atom), nullptr));
            if (owner->owner)
                throw std::runtime_error("composited X11 is unsupported; redirected-window "
                                         "completion is not display delivery");
        }
        auto xwayland = owned(xcb_intern_atom_reply(
            connection, xcb_intern_atom(connection, true, 15, "XWAYLAND_VERSION"), nullptr));
        if (xwayland->atom)
            throw std::runtime_error("Xwayland presentation is unsupported");
        auto resources = owned(xcb_randr_get_screen_resources_current_reply(
            connection, xcb_randr_get_screen_resources_current(connection, root), nullptr));
        const auto *outputs = xcb_randr_get_screen_resources_current_outputs(resources.get());
        unsigned active = 0;
        xcb_randr_output_t selected = 0;
        xcb_randr_crtc_t crtc = 0;
        for (int i = 0; i < xcb_randr_get_screen_resources_current_outputs_length(resources.get());
             ++i) {
            auto output = owned(xcb_randr_get_output_info_reply(
                connection,
                xcb_randr_get_output_info(connection, outputs[i], resources->config_timestamp),
                nullptr));
            if (output->crtc && output->connection == XCB_RANDR_CONNECTION_CONNECTED) {
                active++;
                const std::string name(
                    reinterpret_cast<char *>(xcb_randr_get_output_info_name(output.get())),
                    xcb_randr_get_output_info_name_length(output.get()));
                if (name == argv[1]) {
                    selected = outputs[i];
                    crtc = output->crtc;
                }
            }
        }
        if (active != 1 || !selected)
            throw std::runtime_error(
                "requires exactly one active connected output matching the explicit selection");
        auto info = owned(xcb_randr_get_crtc_info_reply(
            connection, xcb_randr_get_crtc_info(connection, crtc, resources->config_timestamp),
            nullptr));
        if (info->x || info->y || info->width != std::stoi(argv[2]) ||
            info->height != std::stoi(argv[3]) || screens.data->width_in_pixels != info->width ||
            screens.data->height_in_pixels != info->height)
            throw std::runtime_error(
                "selected viewport must match the existing full-screen mode at unit scale");
        auto *modes = xcb_randr_get_screen_resources_current_modes(resources.get());
        double refresh = 0, vertical_blank = 0;
        for (int i = 0; i < xcb_randr_get_screen_resources_current_modes_length(resources.get());
             ++i) {
            const auto &mode = modes[i];
            if (mode.id != info->mode)
                continue;
            if (mode.mode_flags & (XCB_RANDR_MODE_FLAG_INTERLACE | XCB_RANDR_MODE_FLAG_DOUBLE_SCAN))
                throw std::runtime_error("interlaced/doublescan modes are unsupported");
            if (mode.htotal && mode.vtotal && mode.dot_clock) {
                refresh = double(mode.dot_clock) * 1000 / mode.htotal / mode.vtotal;
                vertical_blank = std::ceil(double(mode.vtotal - mode.height) * mode.htotal *
                                           1000000 / mode.dot_clock);
            }
        }
        if (!std::isfinite(refresh) || refresh <= 0)
            throw std::runtime_error("physical refresh timing unavailable");
        std::printf("{\"supported\":true,\"root\":%u,\"outputId\":%u,\"crtc\":%u,\"mode\":%u,"
                    "\"width\":%u,\"height\":%u,\"refreshMillihertz\":%.0f,\"verticalBlankMicros\":%.0f,\"compositor\":false,"
                    "\"backend\":\"X11 / Qt Quick OpenGL / DRI3 Present\"}\n",
                    root, selected, crtc, info->mode, info->width, info->height, refresh, vertical_blank);
        xcb_disconnect(connection);
        return 0;
    } catch (const std::exception &error) {
        std::fprintf(stderr, "%s\n", error.what());
        return 4;
    }
}

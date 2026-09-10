#include "window.h"
#include "shaders.h"
#include <QAbstractAnimation>
#include <QFile>
#include <QGuiApplication>
#include <QImageReader>
#include <QIcon>
#include <QKeyEvent>
#include <QOffscreenSurface>
#include <QOpenGLContext>
#include <QOpenGLExtraFunctions>
#include <QOpenGLShaderProgram>
#include <QQuickWindow>
#include <QQuickItem>
#include <QSGRenderNode>
#include <QScreen>
#include <QSocketNotifier>
#include <QSurfaceFormat>
#include <QThread>
#include <QTimer>
#include <algorithm>
#include <array>
#include <atomic>
#include <cmath>
#include <condition_variable>
#include <cstdio>
#include <cstring>
#include <ctime>
#include <deque>
#include <functional>
#include <future>
#include <jpeglib.h>
#include <memory>
#include <mutex>
#include <setjmp.h>
#include <thread>
#include <vector>

namespace {
using Gl = QOpenGLExtraFunctions;
std::atomic<uint64_t> frameSerial{0};
std::atomic<int64_t> frameTime{0};

struct TextureFormat { GLint internal; GLenum format, type; size_t bytes; };
TextureFormat textureFormat(uint32_t format) {
    switch (format) {
    case 0: return {GL_RGBA8, GL_RGBA, GL_UNSIGNED_BYTE, 4};
    case 1: return {GL_RGBA8, GL_BGRA, GL_UNSIGNED_BYTE, 4};
    case 2: return {GL_R8, GL_RED, GL_UNSIGNED_BYTE, 1};
    case 3: return {GL_RGBA32I, GL_RGBA_INTEGER, GL_INT, 16};
    case 4: return {GL_RGB16I, GL_RGB_INTEGER, GL_SHORT, 6};
    case 5: return {GL_RGBA8, GL_RGBA, GL_UNSIGNED_BYTE, 4};
    default: throw std::runtime_error("Unsupported texture format");
    }
}

struct UploadedTexture { GLuint id; std::shared_ptr<GLsync> ready; };

class UploadWorker {
    struct Allocation { GLuint id; uint32_t width, height, format; };
    std::unique_ptr<QOffscreenSurface> surface;
    std::thread thread;
    std::mutex mutex;
    std::condition_variable changed;
    std::deque<std::function<void(Gl *)>> requests;
    std::vector<Allocation> retired;
    size_t retiredBytes = 0;
    bool stopping = false;
    static constexpr size_t poolLimit = 128 * 1024 * 1024;
public:
    explicit UploadWorker(QOpenGLContext *share) : surface(new QOffscreenSurface) {
        if (!share) throw std::runtime_error("No shared OpenGL context");
        surface->setFormat(share->format());
        surface->create();
        if (!surface->isValid()) throw std::runtime_error("Cannot create upload surface");
        std::promise<bool> ready;
        auto initialized = ready.get_future();
        thread = std::thread([this, share, &ready] {
            QOpenGLContext context;
            context.setFormat(share->format());
            context.setShareContext(share);
            const bool valid = context.create() && context.makeCurrent(surface.get());
            ready.set_value(valid);
            if (!valid) return;
            auto *gl = context.extraFunctions();
            for (;;) {
                std::function<void(Gl *)> request;
                {
                    std::unique_lock lock(mutex);
                    changed.wait(lock, [this] { return stopping || !requests.empty(); });
                    if (requests.empty() && stopping) break;
                    request = std::move(requests.front());
                    requests.pop_front();
                }
                request(gl);
            }
            gl->glFinish();
            for (auto &allocation : retired) gl->glDeleteTextures(1, &allocation.id);
            context.doneCurrent();
        });
        if (!initialized.get()) {
            thread.join();
            throw std::runtime_error("Cannot initialize upload context");
        }
    }
    ~UploadWorker() {
        { std::lock_guard lock(mutex); stopping = true; }
        changed.notify_one();
        thread.join();
    }
    void enqueue(std::function<void(Gl *)> request) {
        { std::lock_guard lock(mutex); requests.push_back(std::move(request)); }
        changed.notify_one();
    }
    std::future<UploadedTexture> upload(uint32_t width, uint32_t height, uint32_t format, const void *pixels) {
        auto ready = std::make_shared<std::promise<UploadedTexture>>();
        auto result = ready->get_future();
        enqueue([this, width, height, format, pixels, ready](Gl *gl) {
            const auto spec = textureFormat(format);
            // Expand opaque input before entering the driver. The upload
            // worker overlaps this conversion with palette sampling.
            QImage expanded;
            const void *uploadPixels = pixels;
            if (format == 5) {
                const QImage source(static_cast<const uint8_t *>(pixels),int(width),int(height),
                                    int((width*3+3)&~3u),QImage::Format_RGB888);
                expanded=source.convertToFormat(QImage::Format_RGBA8888);
                if (expanded.isNull()) { ready->set_value({0,{}}); return; }
                uploadPixels=expanded.constBits();
            }
            GLuint id = 0;
            auto found = std::find_if(retired.begin(), retired.end(), [=](const auto &a) {
                return a.width == width && a.height == height && a.format == format;
            });
            if (found != retired.end()) {
                id = found->id;
                retiredBytes -= size_t(width)*height*spec.bytes;
                retired.erase(found);
            }
            const bool reused = id != 0;
            if (!id) gl->glGenTextures(1, &id);
            gl->glBindTexture(GL_TEXTURE_2D, id);
            const GLint filter = (format == 3 || format == 4) ? GL_NEAREST : GL_LINEAR;
            gl->glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, filter);
            gl->glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, filter);
            gl->glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
            gl->glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
            gl->glPixelStorei(GL_UNPACK_ALIGNMENT, 1);
            if (reused) gl->glTexSubImage2D(GL_TEXTURE_2D, 0, 0, 0, width, height, spec.format, spec.type, uploadPixels);
            else gl->glTexImage2D(GL_TEXTURE_2D, 0, spec.internal, width, height, 0, spec.format, spec.type, uploadPixels);
            if (gl->glGetError() != GL_NO_ERROR) {
                gl->glDeleteTextures(1,&id); ready->set_value({0,{}}); return;
            }
            try { ready->set_value({id,fence(gl)}); }
            catch (...) { gl->glDeleteTextures(1,&id); ready->set_value({0,{}}); }
        });
        return result;
    }
    void release(GLuint id, uint32_t width, uint32_t height, uint32_t format,
                 std::shared_ptr<GLsync> fence) {
        enqueue([this, id, width, height, format, fence=std::move(fence)](Gl *gl) {
            if (fence && *fence) gl->glWaitSync(*fence, 0, GL_TIMEOUT_IGNORED);
            const size_t bytes = size_t(width)*height*textureFormat(format).bytes;
            if (retiredBytes+bytes <= poolLimit) {
                retired.push_back({id,width,height,format});
                retiredBytes += bytes;
            } else gl->glDeleteTextures(1,&id);
        });
    }
    std::shared_ptr<GLsync> fence(Gl *gl) {
        auto sync = gl->glFenceSync(GL_SYNC_GPU_COMMANDS_COMPLETE, 0);
        if (!sync) throw std::runtime_error("Cannot fence rendered textures");
        gl->glFlush();
        // External fence owners also own this worker through their texture or
        // upload. During teardown, queued owners are drained before join ends.
        // Taking another shared worker reference here could otherwise destroy
        // the worker on its own thread when the GUI drops its final reference.
        return {new GLsync(sync), [this](GLsync *value) {
            enqueue([sync=*value](Gl *gl) { gl->glDeleteSync(sync); });
            delete value;
        }};
    }
};

struct Texture {
    GLuint id;
    uint32_t width, height, format;
    std::shared_ptr<UploadWorker> worker;
    std::shared_ptr<GLsync> ready, lastUse;
    ~Texture() { worker->release(id,width,height,format,lastUse ? std::move(lastUse) : std::move(ready)); }
};

struct JpegError { jpeg_error_mgr base; jmp_buf jump; };
bool decodeJpeg(const QByteArray &bytes, QSize size, QImage &pixels) {
    // Allocate C++ objects before the libjpeg error jump. Its error path only
    // crosses C operations; no object requiring destruction is bypassed.
    if (pixels.size() != size || pixels.format() != QImage::Format_RGB888)
        pixels = QImage(size,QImage::Format_RGB888);
    if (pixels.isNull()) return false;
    auto *base = pixels.bits();
    const int stride = pixels.bytesPerLine();
    if (stride>size.width()*3) for (int y=0;y<size.height();++y)
        std::memset(base+y*stride+size.width()*3,0,stride-size.width()*3);
    jpeg_decompress_struct decoder{};
    JpegError error{};
    decoder.err = jpeg_std_error(&error.base);
    error.base.error_exit = [](j_common_ptr common) {
        longjmp(reinterpret_cast<JpegError *>(common->err)->jump,1);
    };
    if (setjmp(error.jump)) { jpeg_destroy_decompress(&decoder); return false; }
    jpeg_create_decompress(&decoder);
    jpeg_mem_src(&decoder,reinterpret_cast<const unsigned char *>(bytes.constData()),bytes.size());
    if (jpeg_read_header(&decoder,TRUE) != JPEG_HEADER_OK
        || decoder.image_width != unsigned(size.width()) || decoder.image_height != unsigned(size.height())
        || (decoder.jpeg_color_space != JCS_YCbCr && decoder.jpeg_color_space != JCS_RGB
            && decoder.jpeg_color_space != JCS_GRAYSCALE)) {
        jpeg_destroy_decompress(&decoder);
        return false;
    }
    decoder.out_color_space = JCS_RGB;
    jpeg_start_decompress(&decoder);
    while (decoder.output_scanline < decoder.output_height) {
        JSAMPROW rows[16];
        const unsigned count = std::min(16u,decoder.output_height-decoder.output_scanline);
        for (unsigned row=0;row<count;++row) rows[row] = base+(decoder.output_scanline+row)*stride;
        jpeg_read_scanlines(&decoder,rows,count);
    }
    jpeg_finish_decompress(&decoder);
    jpeg_destroy_decompress(&decoder);
    return true;
}

QVector4D vector(RsRect r) { return {r.x,r.y,r.width,r.height}; }
QVector4D vector(RsColor c) { return {c.red,c.green,c.blue,c.alpha}; }
class Window : public QQuickWindow {
public:
    RsCallbacks callbacks;
    explicit Window(RsCallbacks input) : callbacks(input) {}
    static int keyCode(int key) {
        return key==Qt::Key_Escape?100:key==Qt::Key_Left?101:key==Qt::Key_Right?102:0;
    }
    void keyPressEvent(QKeyEvent *event) override {
        if (!event->isAutoRepeat() && callbacks.input && keyCode(event->key())) callbacks.input(callbacks.context,keyCode(event->key()));
    }
    void keyReleaseEvent(QKeyEvent *event) override {
        if (!event->isAutoRepeat() && callbacks.input && keyCode(event->key())) callbacks.input(callbacks.context,keyCode(event->key())+100);
    }
    void focusInEvent(QFocusEvent *event) override {
        QQuickWindow::focusInEvent(event);
        if (callbacks.input) callbacks.input(callbacks.context,10);
    }
    void focusOutEvent(QFocusEvent *event) override {
        QQuickWindow::focusOutEvent(event);
        if (callbacks.input) callbacks.input(callbacks.context,11);
    }
};
class Animation : public QAbstractAnimation {
    QQuickWindow &window;
public:
    explicit Animation(QQuickWindow &target) : window(target) {}
    int duration() const override { return -1; }
    void updateCurrentTime(int) override { window.update(); }
};
class SceneNode : public QSGRenderNode {
public:
    std::function<void()> draw;
    void render(const RenderState *) override { draw(); }
    StateFlags changedStates() const override {
        return DepthState | StencilState | ScissorState | ColorState | BlendState | CullState | ViewportState;
    }
};
class SceneItem : public QQuickItem {
public:
    std::function<void()> draw;
    explicit SceneItem(QQuickItem *parent) : QQuickItem(parent) { setFlag(ItemHasContents); }
    QSGNode *updatePaintNode(QSGNode *previous,UpdatePaintNodeData *) override {
        auto *node=static_cast<SceneNode *>(previous);
        if (!node) node=new SceneNode;
        node->draw=draw;
        return node;
    }
};
struct Scene {
    uint64_t serial = 0;
    std::vector<RsGraphic> graphics;
    std::vector<RsSprite> sprites;
    std::vector<std::shared_ptr<Texture>> textures;
    std::vector<std::array<GLuint,3>> graphicTextures;
    std::vector<bool> opaqueArtwork;
    std::vector<std::array<GLuint,2>> spriteTextures;
};
}

struct RsTexture { std::shared_ptr<Texture> texture; };
struct RsUpload {
    std::shared_ptr<UploadWorker> worker;
    std::future<UploadedTexture> ready;
    uint32_t width, height, format;
};
struct RsWindow {
    int argc = 1;
    char name[22] = "io.roonscape.Renderer";
    char *argv[2] = {name,nullptr};
    std::unique_ptr<QGuiApplication> application;
    std::unique_ptr<Window> window;
    std::unique_ptr<Animation> animation;
    SceneItem *item = nullptr;
    std::shared_ptr<UploadWorker> worker;
    std::shared_ptr<Scene> pending, current;
    std::unique_ptr<QOpenGLShaderProgram> graphics, sharedGraphics, sprites;
    GLuint vao = 0;
    QByteArray error;
    std::atomic<bool> captureRequested{false};
    std::atomic<bool> animationWanted{false};
    std::mutex captureMutex;
    std::vector<uint8_t> captured;
    RsCallbacks callbacks;
    std::mutex renderCallbackMutex;
    QSize renderSize;
    double renderScale = 1.0;
    uint32_t renderRate = 60000;
    uint64_t nextSerial = 0;
    std::atomic<uint64_t> renderedSerial{0};

    RsWindow(uint32_t width,uint32_t height,bool fullscreen,RsCallbacks input) : callbacks(input) {
        QCoreApplication::setAttribute(Qt::AA_ShareOpenGLContexts);
        QSurfaceFormat format;
        format.setVersion(3,3);
        format.setProfile(QSurfaceFormat::CoreProfile);
        format.setAlphaBufferSize(0);
        format.setSwapInterval(1);
        QSurfaceFormat::setDefaultFormat(format);
        QQuickWindow::setGraphicsApi(QSGRendererInterface::OpenGL);
        application = std::make_unique<QGuiApplication>(argc,argv);
        application->setApplicationName("io.roonscape.Renderer");
        application->setDesktopFileName("io.roonscape.Renderer");
        worker = std::make_shared<UploadWorker>(QOpenGLContext::globalShareContext());
        window = std::make_unique<Window>(callbacks);
        window->setTitle("RoonScape");
        window->setColor(Qt::black);
        window->resize(int(width),int(height));
        item=new SceneItem(window->contentItem());
        item->setSize(window->size());
        QObject::connect(window.get(),&QQuickWindow::widthChanged,item,[this] { item->setWidth(window->width()); });
        QObject::connect(window.get(),&QQuickWindow::heightChanged,item,[this] { item->setHeight(window->height()); });
        item->draw=[this] {
            try { paint(); } catch (const std::exception &e) { fail(e.what()); }
        };
        animation = std::make_unique<Animation>(*window);
        QObject::connect(window.get(),&QQuickWindow::beforeFrameBegin,window.get(),[] {
            frameTime.store(rs_clock_micros(),std::memory_order_relaxed);
            frameSerial.fetch_add(1,std::memory_order_relaxed);
        },Qt::DirectConnection);
        QObject::connect(window.get(),&QQuickWindow::afterAnimating,window.get(),[this] {
            const auto rate = uint32_t(std::llround(window->screen()->refreshRate()*1000));
            if (callbacks.update) callbacks.update(callbacks.context,rs_clock_micros(),window->width(),window->height(),rate);
        });
        QObject::connect(window.get(),&QQuickWindow::beforeSynchronizing,window.get(),[this] {
            // Qt holds the GUI thread here, making window properties safe to copy.
            renderSize = window->size();
            renderScale = window->devicePixelRatio();
            renderRate = uint32_t(std::llround(window->screen()->refreshRate()*1000));
            if (pending) current = std::move(pending);
        },Qt::DirectConnection);
        QObject::connect(window.get(),&QQuickWindow::beforeRendering,window.get(),[this] {
            try { initialize(); } catch (const std::exception &e) { fail(e.what()); }
        },Qt::DirectConnection);
        QObject::connect(window.get(),&QQuickWindow::frameSwapped,window.get(),[this] {
            const auto serial=renderedSerial.load(std::memory_order_relaxed);
            const auto frame=frameSerial.load(std::memory_order_relaxed);
            const auto time=frameTime.load(std::memory_order_relaxed);
            QMetaObject::invokeMethod(window.get(),[this,serial,frame,time] {
                if (callbacks.painted) callbacks.painted(callbacks.context,serial,frame,time);
            },Qt::QueuedConnection);
        },Qt::DirectConnection);
        QObject::connect(window.get(),&QQuickWindow::sceneGraphInvalidated,window.get(),[this] {
            cleanup();
        },Qt::DirectConnection);
        if (fullscreen) {
            // Bare X11 sessions have no window manager to honor the state hint.
            if (application->screens().size()==1) window->setGeometry(window->screen()->geometry());
            window->showFullScreen();
        } else window->show();
    }
    ~RsWindow() {
        animation->stop();
        window->hide();
        window.reset();
        current.reset();
        pending.reset();
        worker.reset();
    }
    void fail(const char *message) {
        const QByteArray detail(message);
        QMetaObject::invokeMethod(window.get(),[this,detail] {
            error = detail;
            application->exit(1);
        },Qt::QueuedConnection);
    }
    std::unique_ptr<QOpenGLShaderProgram> program(const char *vertex,const char *fragment) {
        auto result = std::make_unique<QOpenGLShaderProgram>();
        if (!result->addShaderFromSourceCode(QOpenGLShader::Vertex,vertex)
            || !result->addShaderFromSourceCode(QOpenGLShader::Fragment,fragment) || !result->link())
            throw std::runtime_error(result->log().toStdString());
        return result;
    }
    void initialize() {
        if (graphics) return;
        const char *graphicsVertex=R"GLSL(#version 330 core
            out vec2 position;
            void main() {
                vec2 p = vec2((gl_VertexID << 1)&2,gl_VertexID&2);
                gl_Position = vec4(p*2.0-1.0,0,1);
                position = vec2(p.x,1.0-p.y);
            }
        )GLSL";
        graphics = program(graphicsVertex,graphics_shader);
        QByteArray sharedSource(graphics_shader);
        sharedSource.replace("#version 330 core\n","#version 330 core\n#define SHARED_GEOMETRY\n");
        sharedGraphics = program(graphicsVertex,sharedSource.constData());
        sprites = program(R"GLSL(#version 330 core
            uniform vec2 viewport;
            uniform vec4 bounds;
            uniform float angle;
            out vec2 position;
            void main() {
                vec2 corner = vec2((gl_VertexID&1)==1?1.0:-1.0,(gl_VertexID&2)==2?1.0:-1.0);
                vec2 d = corner*(bounds.zw*0.5+vec2(1));
                float c = cos(angle),s = sin(angle);
                position = bounds.xy+bounds.zw*0.5+mat2(c,s,-s,c)*d;
                gl_Position = vec4(position/viewport*vec2(2,-2)+vec2(-1,1),0,1);
            }
        )GLSL",sprite_shader);
        QOpenGLContext::currentContext()->extraFunctions()->glGenVertexArrays(1,&vao);
    }
    void cleanup() {
        current.reset();
        graphics.reset();
        sharedGraphics.reset();
        sprites.reset();
        if (vao) QOpenGLContext::currentContext()->extraFunctions()->glDeleteVertexArrays(1,&vao);
        vao = 0;
    }
    void paint() {
        {
            std::lock_guard lock(renderCallbackMutex);
            if (callbacks.render) callbacks.render(callbacks.context,frameTime.load(std::memory_order_relaxed),
                renderSize.width(),renderSize.height(),renderRate,renderScale);
        }
        if (!current || !graphics || !sprites) return;
        auto *gl = QOpenGLContext::currentContext()->extraFunctions();
        for (auto &texture : current->textures) {
            if (texture->ready) {
                // Order the first draw after upload without blocking either
                // CPU thread or exposing partially uploaded image contents.
                gl->glWaitSync(*texture->ready,0,GL_TIMEOUT_IGNORED);
                texture->ready.reset();
            }
        }
        const QSize physical = renderSize*renderScale;
        gl->glViewport(0,0,physical.width(),physical.height());
        gl->glDisable(GL_DEPTH_TEST);
        gl->glDisable(GL_STENCIL_TEST);
        gl->glDisable(GL_SCISSOR_TEST);
        gl->glDisable(GL_CULL_FACE);
        gl->glDisable(GL_BLEND);
        gl->glColorMask(GL_TRUE,GL_TRUE,GL_TRUE,GL_TRUE);
        gl->glBindVertexArray(vao);
        for (size_t batch=0;batch<current->graphics.size();batch+=3) {
            const auto &first=current->graphics[batch];
            bool shared=first.artwork_bounds.width>0 && first.artwork_bounds.height>0;
            for (size_t i=batch;i<std::min(batch+3,current->graphics.size());++i) {
                const auto &g=current->graphics[i];
                shared=shared && current->opaqueArtwork[i]
                    && vector(g.canvas)==vector(first.canvas)
                    && vector(g.artwork_bounds)==vector(first.artwork_bounds)
                    && vector(g.plate_bounds)==vector(first.plate_bounds)
                    && g.border_width==first.border_width
                    && g.shadow_radius==first.shadow_radius && g.shadow_y==first.shadow_y;
            }
            auto *shader=shared?sharedGraphics.get():graphics.get();
            shader->bind();
            shader->setUniformValue("viewport",QVector2D(renderSize.width(),renderSize.height()));
            shader->setUniformValue("physicalViewport",QVector2D(physical.width(),physical.height()));
            shader->setUniformValue("noise",6);
            for (size_t i=0;i<3;++i) {
                shader->setUniformValue(("art"+QByteArray::number(i)).constData(),int(i*2));
                shader->setUniformValue(("gradient"+QByteArray::number(i)).constData(),int(i*2+1));
                const QByteArray prefix = "graphics["+QByteArray::number(i)+"].";
                const auto set = [&](const char *name,auto value) {
                    shader->setUniformValue((prefix+name).constData(),value);
                };
                if (batch+i>=current->graphics.size()) { set("weight",0.f); continue; }
                const auto &g = current->graphics[batch+i];
                const auto &textures = current->graphicTextures[batch+i];
                if (!shared || i==0) {
                    set("canvas",vector(g.canvas));
                    set("bounds",vector(g.artwork_bounds)); set("plateBounds",vector(g.plate_bounds));
                    set("borderWidth",g.border_width); set("shadowRadius",g.shadow_radius);
                    set("shadowY",g.shadow_y);
                }
                set("background",vector(g.background)); set("border",vector(g.border));
                set("plate",vector(g.plate)); set("quiet",vector(g.quiet));
                if (!shared) { set("muted",vector(g.muted)); set("hasArtwork",textures[0]!=0); }
                set("shadowAlpha",g.shadow_alpha); set("weight",g.weight);
                set("hasGradient",textures[1]!=0);
                gl->glUniform3ui(shader->uniformLocation((prefix+"steps").constData()),g.origin,g.step_x,g.step_y);
                gl->glActiveTexture(GL_TEXTURE0+i*2); gl->glBindTexture(GL_TEXTURE_2D,textures[0]);
                gl->glActiveTexture(GL_TEXTURE0+i*2+1); gl->glBindTexture(GL_TEXTURE_2D,textures[1]);
                gl->glActiveTexture(GL_TEXTURE6); gl->glBindTexture(GL_TEXTURE_2D,textures[2]);
            }
            if (batch) { gl->glEnable(GL_BLEND); gl->glBlendFunc(GL_ONE,GL_ONE); gl->glColorMask(GL_TRUE,GL_TRUE,GL_TRUE,GL_FALSE); }
            gl->glDrawArrays(GL_TRIANGLES,0,3);
            shader->release();
        }
        gl->glColorMask(GL_TRUE,GL_TRUE,GL_TRUE,GL_TRUE);
        gl->glEnable(GL_BLEND);
        gl->glBlendFunc(GL_ONE,GL_ONE_MINUS_SRC_ALPHA);
        sprites->bind();
        sprites->setUniformValue("viewport",QVector2D(renderSize.width(),renderSize.height()));
        sprites->setUniformValue("image",0);
        sprites->setUniformValue("foreground",1);
        for (size_t i=0;i<current->sprites.size();++i) {
            const auto &s = current->sprites[i];
            if (s.color.alpha<=0 || s.bounds.width<=0 || s.bounds.height<=0) continue;
            gl->glActiveTexture(GL_TEXTURE0);
            gl->glBindTexture(GL_TEXTURE_2D,current->spriteTextures[i][0]);
            gl->glActiveTexture(GL_TEXTURE1);
            gl->glBindTexture(GL_TEXTURE_2D,current->spriteTextures[i][1]);
            sprites->setUniformValue("bounds",vector(s.bounds)); sprites->setUniformValue("uv",vector(s.uv));
            sprites->setUniformValue("clip",vector(s.clip)); sprites->setUniformValue("tint",vector(s.color));
            sprites->setUniformValue("secondary",vector(s.secondary));
            sprites->setUniformValue("radius",s.radius); sprites->setUniformValue("angle",s.angle);
            sprites->setUniformValue("fadeTop",s.fade_top); sprites->setUniformValue("fadeBottom",s.fade_bottom);
            sprites->setUniformValue("dimming",s.dimming);
            sprites->setUniformValue("kind",GLint(s.kind));
            gl->glDrawArrays(GL_TRIANGLE_STRIP,0,4);
        }
        sprites->release();
        gl->glBindVertexArray(0);
        if (captureRequested.exchange(false)) {
            std::vector<uint8_t> pixels(size_t(physical.width())*physical.height()*4);
            gl->glPixelStorei(GL_PACK_ALIGNMENT,1);
            gl->glReadPixels(0,0,physical.width(),physical.height(),GL_RGBA,GL_UNSIGNED_BYTE,pixels.data());
            std::lock_guard lock(captureMutex);
            captured.resize(pixels.size());
            const size_t stride=size_t(physical.width())*4;
            for (int y=0;y<physical.height();++y)
                std::memcpy(captured.data()+size_t(y)*stride,pixels.data()+size_t(physical.height()-y-1)*stride,stride);
        }
        auto fence = worker->fence(gl);
        for (auto &texture : current->textures) texture->lastUse = fence;
        renderedSerial.store(current->serial,std::memory_order_relaxed);
    }
};

struct RsImage { QImage pixels; bool hasAlpha; };

extern "C" {
uint64_t roonscape_frame_serial() { return frameSerial.load(std::memory_order_relaxed); }
int64_t roonscape_frame_time_micros() { return frameTime.load(std::memory_order_relaxed); }
int64_t rs_clock_micros() {
    timespec time{};
    clock_gettime(CLOCK_MONOTONIC,&time);
    return time.tv_sec*1000000LL+time.tv_nsec/1000;
}
RsWindow *rs_window_new(uint32_t width,uint32_t height,bool fullscreen,RsCallbacks callbacks) {
    try { return new RsWindow(width,height,fullscreen,callbacks); }
    catch (const std::exception &error) { std::fprintf(stderr,"RoonScape window: %s\n",error.what()); return nullptr; }
}
int rs_window_run(RsWindow *window) { return window->application->exec(); }
void rs_window_callbacks(RsWindow *window,RsCallbacks callbacks) {
    std::lock_guard lock(window->renderCallbackMutex);
    window->callbacks=callbacks;
    window->window->callbacks=callbacks;
}
void rs_window_delete(RsWindow *window) { delete window; }
void rs_window_quit(RsWindow *window) {
    QMetaObject::invokeMethod(window->window.get(),[window] { window->application->quit(); },Qt::QueuedConnection);
}
void rs_window_wake(RsWindow *window) {
    QMetaObject::invokeMethod(window->window.get(),[window] { window->window->update(); },Qt::QueuedConnection);
}
void rs_window_apply_animation(RsWindow *window) {
    const bool active = window->animationWanted.load(std::memory_order_acquire);
    if (active && window->animation->state()!=QAbstractAnimation::Running) window->animation->start();
    else if (!active) window->animation->stop();
}
void rs_window_animate(RsWindow *window,bool active) {
    window->animationWanted.store(active,std::memory_order_release);
    rs_window_apply_animation(window);
}
void rs_window_render_animate(RsWindow *window,bool active) {
    window->animationWanted.store(active,std::memory_order_release);
    // A later GUI request can supersede this queued render-thread request.
    QMetaObject::invokeMethod(window->window.get(),[window] { rs_window_apply_animation(window); },Qt::QueuedConnection);
}
void rs_window_timer(RsWindow *window,uint32_t milliseconds,int32_t event) {
    auto *timer=new QTimer(window->window.get());
    QObject::connect(timer,&QTimer::timeout,window->window.get(),[window,event] {
        if (window->callbacks.input) window->callbacks.input(window->callbacks.context,event);
    });
    timer->start(int(milliseconds));
}
void rs_window_icon(RsWindow *window,const uint8_t *rgba,uint32_t width,uint32_t height) {
    QImage image(rgba,int(width),int(height),int(width*4),QImage::Format_RGBA8888);
    window->window->setIcon(QIcon(QPixmap::fromImage(image)));
}
void rs_window_size(RsWindow *window,uint32_t *width,uint32_t *height) { *width=window->window->width(); *height=window->window->height(); }
double rs_window_scale(RsWindow *window) { return window->window->devicePixelRatio(); }
void rs_window_resize(RsWindow *window,uint32_t width,uint32_t height) { window->window->resize(width,height); }
void rs_window_watch(RsWindow *window,int fd,int32_t event) {
    auto *notifier = new QSocketNotifier(fd,QSocketNotifier::Read,window->window.get());
    QObject::connect(notifier,&QSocketNotifier::activated,window->window.get(),[window,event] {
        window->callbacks.input(window->callbacks.context,event);
        window->window->update();
    });
}
uint64_t rs_window_scene(RsWindow *window,const RsGraphic *graphics,size_t graphicCount,const RsSprite *sprites,size_t spriteCount,bool direct) {
    auto scene = std::make_shared<Scene>();
    scene->serial = ++window->nextSerial;
    const auto retain = [&](const RsTexture *texture) -> GLuint {
        if (!texture) return 0;
        scene->textures.push_back(texture->texture);
        return texture->texture->id;
    };
    for (size_t i=0;i<graphicCount;++i) {
        scene->graphics.push_back(graphics[i]);
        scene->graphicTextures.push_back({retain(graphics[i].artwork),retain(graphics[i].gradient),retain(graphics[i].noise)});
        scene->opaqueArtwork.push_back(graphics[i].artwork && graphics[i].artwork->texture->format==5);
    }
    for (size_t i=0;i<spriteCount;++i) {
        scene->sprites.push_back(sprites[i]);
        scene->spriteTextures.push_back({retain(sprites[i].texture),retain(sprites[i].foreground)});
    }
    if (direct) window->current = std::move(scene);
    else {
        window->pending = std::move(scene);
        window->item->update();
        window->window->update();
    }
    return window->nextSerial;
}
RsUpload *rs_texture_upload_start(RsWindow *window,uint32_t width,uint32_t height,uint32_t format,const void *pixels) {
    try {
        textureFormat(format);
        return new RsUpload{window->worker,window->worker->upload(width,height,format,pixels),width,height,format};
    } catch (const std::exception &error) { window->fail(error.what()); return nullptr; }
}
RsTexture *rs_texture_upload_finish(RsUpload *pending) {
    if (!pending) return nullptr;
    std::unique_ptr<RsUpload> upload(pending);
    auto prepared = upload->ready.get();
    if (!prepared.id) return nullptr;
    auto texture = std::make_shared<Texture>();
    texture->ready=std::move(prepared.ready);
    texture->id=prepared.id; texture->width=upload->width; texture->height=upload->height; texture->format=upload->format;
    texture->worker=upload->worker;
    return new RsTexture{std::move(texture)};
}
RsTexture *rs_texture_upload(RsWindow *window,uint32_t width,uint32_t height,uint32_t format,const void *pixels) {
    return rs_texture_upload_finish(rs_texture_upload_start(window,width,height,format,pixels));
}
RsImage *rs_image_decode(const char *path,uint32_t *width,uint32_t *height) {
    QImageReader reader(QString::fromUtf8(path));
    const QSize size = reader.size();
    static thread_local QImage pixels;
    bool decoded = false;
    if (reader.format()=="jpeg" && size.isValid()) {
        QFile file(QString::fromUtf8(path));
        if (file.open(QIODevice::ReadOnly)) decoded=decodeJpeg(file.readAll(),size,pixels);
    }
    if (!decoded && !reader.read(&pixels)) return nullptr;
    const bool hasAlpha = !decoded && pixels.hasAlphaChannel();
    if (!decoded) pixels=pixels.convertToFormat(QImage::Format_RGBA8888);
    if (pixels.isNull()) return nullptr;
    *width=pixels.width(); *height=pixels.height();
    return new RsImage{pixels,hasAlpha};
}
RsImage *rs_image_palette(const RsImage *image) {
    QImage pixels=image->pixels.convertToFormat(image->hasAlpha ? QImage::Format_RGBA8888 : QImage::Format_RGB888);
    if (pixels.isNull()) return nullptr;
    const int used=pixels.width()*(image->hasAlpha ? 4 : 3);
    if (pixels.bytesPerLine()>used) for (int y=0;y<pixels.height();++y)
        std::memset(pixels.scanLine(y)+used,0,pixels.bytesPerLine()-used);
    return new RsImage{pixels,image->hasAlpha};
}
uint32_t rs_image_channels(const RsImage *image) { return image->pixels.format()==QImage::Format_RGB888 ? 3 : 4; }
uint32_t rs_image_stride(const RsImage *image) { return image->pixels.bytesPerLine(); }
bool rs_image_has_alpha(const RsImage *image) { return image->hasAlpha; }
const uint8_t *rs_image_pixels(const RsImage *image) { return image->pixels.constBits(); }
void rs_image_delete(RsImage *image) { delete image; }
RsTexture *rs_texture_clone(const RsTexture *texture) { return new RsTexture{texture->texture}; }
void rs_texture_delete(RsTexture *texture) { delete texture; }
const char *rs_window_error(RsWindow *window) { return window->error.constData(); }
bool rs_window_capture(RsWindow *window,uint8_t *output,size_t length) {
    std::lock_guard lock(window->captureMutex);
    if (window->captured.size()!=length) return false;
    std::memcpy(output,window->captured.data(),length);
    window->captured.clear();
    return true;
}
void rs_window_request_capture(RsWindow *window) {
    window->captureRequested=true;
    window->window->update();
}
}

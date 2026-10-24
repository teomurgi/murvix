#[cfg(target_os = "macos")]
#[allow(unsafe_code, clippy::expect_used)]
mod probe {
    use gpui::{
        AppContext, Application, Context, IntoElement, Render, Window, WindowKind, WindowOptions,
        div,
    };
    use murvix_ui::webview::{NativeWebview, assets::Source};
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2_app_kit::NSView;
    use objc2_web_kit::WKWebView;
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    struct Probe;

    impl Render for Probe {
        fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    pub(super) fn run() {
        Application::new().run(|cx| {
            for kind in [WindowKind::Normal, WindowKind::PopUp, WindowKind::Normal] {
                cx.open_window(
                    WindowOptions {
                        kind,
                        focus: false,
                        show: false,
                        ..Default::default()
                    },
                    |window, cx| {
                        let RawWindowHandle::AppKit(handle) =
                            window.window_handle().expect("handle").as_raw()
                        else {
                            unreachable!()
                        };
                        let parent =
                            unsafe { Retained::retain(handle.ns_view.as_ptr().cast::<NSView>()) }
                                .expect("parent");
                        let native_window = parent.window().expect("native window");
                        let mtm = MainThreadMarker::new().expect("main thread");
                        let observer = unsafe { WKWebView::new(mtm) };
                        parent.addSubview(&observer);
                        let observed_class = native_window.class();
                        assert!(
                            observed_class
                                .name()
                                .to_bytes()
                                .starts_with(b"NSKVONotifying_")
                        );
                        for _ in 0..3 {
                            let source = Source {
                                owner: "file-drop-probe".into(),
                                directory: std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                                    .join("examples/fixtures"),
                                entry: "webview_keyboard.html".into(),
                            };
                            let (page, _events) = NativeWebview::new(
                                window,
                                source,
                                "",
                                gpui::rgb(0x00ff_ffff),
                                false,
                            )
                            .expect("extension webview");
                            assert_eq!(native_window.class(), observed_class);
                            drop(page);
                            assert_eq!(native_window.class(), observed_class);
                        }
                        observer.removeFromSuperview();
                        cx.new(|_| Probe)
                    },
                )
                .expect("window");
            }
            cx.quit();
        });
    }
}

fn main() {
    #[cfg(target_os = "macos")]
    probe::run();
}

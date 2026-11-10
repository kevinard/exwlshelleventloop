use super::*;

// Exercise the adapter's action consumer with no remaining windows, as after
// output removal. No compositor, display connection or rendering is needed.
struct EmptyDaemon;

#[derive(Debug)]
enum Message {
    OutputsChanged,
}

impl From<WaylandOutputSnapshot> for Message {
    fn from(_: WaylandOutputSnapshot) -> Self {
        Self::OutputsChanged
    }
}

impl TryInto<LayerShellCustomActionWithId> for Message {
    type Error = Self;
    fn try_into(self) -> Result<LayerShellCustomActionWithId, Self> {
        Err(self)
    }
}

impl IcedProgram for EmptyDaemon {
    type State = ();
    type Message = Message;
    type Theme = iced_core::Theme;
    type Renderer = iced_renderer::Renderer;
    type Executor = iced_futures::backend::default::Executor;

    fn name() -> &'static str {
        "retired-window-test"
    }
    fn settings(&self) -> iced_core::Settings {
        Default::default()
    }
    fn window(&self) -> Option<iced_core::window::Settings> {
        None
    }
    fn boot(&self) -> ((), iced_runtime::Task<Message>) {
        ((), iced_runtime::Task::none())
    }
    fn update(&self, _: &mut (), message: Message) -> iced_runtime::Task<Message> {
        let _ = message;
        iced_runtime::Task::none()
    }
    fn view<'a>(
        &self,
        _: &'a (),
        _: IcedId,
    ) -> iced_core::Element<'a, Message, Self::Theme, Self::Renderer> {
        unreachable!("no window can request a view")
    }
}

#[test]
fn closing_retired_windows_does_not_leave_retriable_adapter_work() {
    let (sender, _receiver) = layershellev::calloop::channel::channel();
    let proxy = IcedProxy::new(sender);
    let runtime = Runtime::new(
        <EmptyDaemon as IcedProgram>::Executor::new().unwrap(),
        proxy.clone(),
    );
    let (application, _) = Instance::new(EmptyDaemon);
    let mut context: Context<EmptyDaemon, _, iced_renderer::Compositor> = Context::new(
        application,
        Default::default(),
        runtime,
        vec![],
        iced_core::theme::Mode::Dark,
        proxy,
    );
    let mut backend = WindowState::new("retired-window-test");
    for _ in 0..100 {
        let completed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let callback_completed = completed.clone();
        context.waiting_layer_shell_actions.push((
            Some(IcedId::unique()),
            LayerShellCustomAction::SetIdleInhibition {
                enabled: true,
                callback: crate::actions::IdleInhibitionCallback(std::sync::Arc::new(
                    move |available| {
                        assert!(!available, "retired windows cannot inhibit idle");
                        callback_completed.store(true, std::sync::atomic::Ordering::SeqCst);
                    },
                )),
            },
        ));
        context.handle_user_action(
            &mut backend,
            Action::Window(iced_runtime::window::Action::Close(IcedId::unique())),
        );
        let (next, _) =
            context.handle_event(&mut backend, None, IcedLayerShellEvent::NormalDispatch);
        let ContextState::Context(next) = next else {
            panic!("closing an absent window must not initialize graphics")
        };
        context = next;
        assert!(completed.load(std::sync::atomic::Ordering::SeqCst));
    }
    assert!(
        context.waiting_layer_shell_actions.is_empty(),
        "completed closes must not retain work: {}",
        context.waiting_layer_shell_actions.len()
    );
}

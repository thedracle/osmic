use osmic_app::{App, Plugin};

#[derive(Debug)]
struct StartupMessage {
    value: &'static str,
}

struct StartupPlugin;

impl Plugin for StartupPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(StartupMessage {
            value: "custom plugin initialized",
        });
    }

    fn name(&self) -> &str {
        "StartupPlugin"
    }
}

fn main() {
    let mut app = App::new();
    app.add_plugin(StartupPlugin);
    app.build();

    if let Some(message) = app.get_resource::<StartupMessage>() {
        println!("{}", message.value);
    }

    app.cleanup();
}

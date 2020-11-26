use arugio_shared::{ClientMessage, ServerMessage};
use bevy::prelude::*;
use bevy_networking_turbulence::{NetworkEvent, NetworkResource, NetworkingPlugin};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

fn main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Debug).expect("cannot initialize console_log");

    App::build()
        .add_plugins(bevy_webgl2::DefaultPlugins)
        .add_plugin(NetworkingPlugin)
        .add_resource(EventReader::<NetworkEvent>::default())
        .add_startup_system(arugio_shared::network_channels_setup)
        .add_startup_system(setup_world_system)
        .add_startup_system(client_setup_system)
        .add_system(handle_network_events_system)
        .add_system(keyboard_input_system)
        .add_system_to_stage(stage::PRE_UPDATE, read_network_channels_system)
        .run();
}

fn setup_world_system(
    commands: &mut Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands
        .spawn(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Cube { size: 1.0 })),
            material: materials.add(Color::rgb(1.0, 0.0, 0.0).into()),
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, 0.5)),
            ..Default::default()
        })
        .spawn(LightBundle {
            transform: Transform::from_translation(Vec3::new(11.0, -15.0, 10.0)),
            ..Default::default()
        })
        .spawn(Camera3dBundle {
            transform: Transform::from_translation(Vec3::new(-10.0, -10.0, 10.0))
                .looking_at(Vec3::default(), Vec3::unit_z()),
            ..Default::default()
        });
}

fn client_setup_system(mut net: ResMut<NetworkResource>) {
    let ip_address = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let socket_address = SocketAddr::new(ip_address, 9001);
    info!("Connecting to {}...", socket_address);
    net.connect(socket_address);
}

fn keyboard_input_system(keyboard_input: Res<Input<KeyCode>>) {
    let pressed = keyboard_input.get_just_pressed();
    for key in pressed {
        info!("Keyboard input: {:?}", key);
    }
}

fn read_network_channels_system(mut net: ResMut<NetworkResource>) {
    for (_handle, connection) in net.connections.iter_mut() {
        let channels = connection.channels().unwrap();

        while let Some(message) = channels.recv::<ServerMessage>() {
            info!("Received message: {:?}", message);
        }
    }
}

fn handle_network_events_system(
    _commands: &mut Commands,
    mut net: ResMut<NetworkResource>,
    network_events: Res<Events<NetworkEvent>>,
    mut network_event_reader: ResMut<EventReader<NetworkEvent>>,
) {
    for event in network_event_reader.iter(&network_events) {
        match event {
            NetworkEvent::Connected(handle) => match net.connections.get_mut(handle) {
                Some(_connection) => {
                    info!("Connection successful");
                    match net.send_message(*handle, ClientMessage::Ping) {
                        Ok(msg) => match msg {
                            Some(msg) => {
                                error!("Unsent message {:?}", msg);
                            }
                            None => {}
                        },
                        Err(err) => {
                            error!("Could not send message: {}", err);
                        }
                    };
                }
                None => panic!("Got packet for non-existing connection [{}]", handle),
            },
            _ => {}
        }
    }
}

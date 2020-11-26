use arugio_shared::{ClientMessage, ServerMessage};
use bevy::prelude::*;
use bevy::{
    app::ScheduleRunnerSettings,
    prelude::App,
    prelude::{EventReader, ResMut},
    MinimalPlugins,
};
use bevy_networking_turbulence::{NetworkEvent, NetworkResource, NetworkingPlugin};
use std::{net::IpAddr, net::Ipv4Addr, net::SocketAddr, time::Duration};

fn main() {
    App::build()
        .add_resource(ScheduleRunnerSettings::run_loop(Duration::from_millis(
            1000 / 60,
        )))
        .add_plugins(MinimalPlugins)
        .add_plugin(NetworkingPlugin)
        .add_resource(EventReader::<NetworkEvent>::default())
        .add_startup_system(arugio_shared::network_channels_setup)
        .add_startup_system(server_setup_system)
        .add_system(handle_network_events_system)
        .add_system_to_stage(stage::PRE_UPDATE, read_network_channels_system)
        .run();
}

fn server_setup_system(mut net: ResMut<NetworkResource>) {
    let ip_address = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let socket_address = SocketAddr::new(ip_address, 9001);
    net.listen(socket_address);
    println!("Listening...");
}

fn read_network_channels_system(mut net: ResMut<NetworkResource>) {
    for (_handle, connection) in net.connections.iter_mut() {
        let channels = connection.channels().unwrap();

        while let Some(message) = channels.recv::<ClientMessage>() {
            println!("Received message: {:?}", message);
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
                    println!("New connection handle: {:?}", handle);
                    net.send_message(*handle, ServerMessage::Pong)
                        .expect("Could not send pong!");
                }
                None => panic!("Got packet for non-existing connection [{}]", handle),
            },
            _ => {}
        }
    }
}

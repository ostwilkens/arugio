use arugio_shared::{BallId, ClientMessage, Position, ServerMessage, TargetVelocity, Velocity};
use bevy::{
    app::ScheduleRunnerSettings,
    prelude::App,
    prelude::{EventReader, ResMut},
    MinimalPlugins,
};
use bevy::{math::vec2, prelude::*};
use bevy_networking_turbulence::{NetworkEvent, NetworkResource, NetworkingPlugin};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, net::IpAddr, net::Ipv4Addr, net::SocketAddr, time::Duration};
use turbulence::message_channels::ChannelMessage;

#[derive(Serialize, Deserialize)]
struct NetworkHandle(u32);

fn main() {
    App::build()
        .add_resource(ScheduleRunnerSettings::run_loop(Duration::from_millis(
            1000 / 30,
        )))
        .add_plugins(MinimalPlugins)
        .add_plugin(NetworkingPlugin::default())
        .add_resource(EventReader::<NetworkEvent>::default())
        .add_startup_system(arugio_shared::network_channels_setup.system())
        .add_startup_system(server_setup_system.system())
        .add_system(handle_network_events_system.system())
        .add_system(arugio_shared::update_velocity_system.system())
        .add_system(arugio_shared::update_position_system.system())
        .add_system(spawn_ball_system.system())
        .add_system(unowned_ball_input_system.system())
        .add_system_to_stage(
            stage::PRE_UPDATE,
            read_component_channel_system::<Position>.system(),
        )
        .add_system_to_stage(
            stage::PRE_UPDATE,
            read_component_channel_system::<TargetVelocity>.system(),
        )
        .add_system_to_stage(stage::PRE_UPDATE, read_network_channels_system.system())
        .add_system_to_stage(stage::POST_UPDATE, broadcast_changes_system.system())
        .run();
}

fn server_setup_system(mut net: ResMut<NetworkResource>) {
    let ip_address = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let socket_address = SocketAddr::new(ip_address, 9001);
    net.listen(socket_address);
    println!("Listening...");
}

fn read_network_channels_system(mut net: ResMut<NetworkResource>) {
    for (_, connection) in net.connections.iter_mut() {
        let channels = connection.channels().unwrap();

        while let Some(message) = channels.recv::<ClientMessage>() {
            println!("Received message: {:?}", message);
        }
    }
}

fn handle_network_events_system(
    cmd: &mut Commands,
    mut net: ResMut<NetworkResource>,
    network_events: Res<Events<NetworkEvent>>,
    mut network_event_reader: ResMut<EventReader<NetworkEvent>>,
    unowned_balls: Query<(Entity, &BallId), Without<NetworkHandle>>,
) {
    for event in network_event_reader.iter(&network_events) {
        match event {
            NetworkEvent::Connected(handle) => match net.connections.get_mut(handle) {
                Some(_connection) => {
                    println!("New connection handle: {:?}", &handle);

                    let (entity, ball) = unowned_balls.iter().next().expect("No unowned balls");
                    cmd.insert_one(entity, NetworkHandle(*handle));
                    net.send_message(*handle, ServerMessage::Welcome(*ball))
                        .expect("Could not send welcome");
                }
                None => panic!("Got packet for non-existing connection [{}]", handle),
            },
            _ => {}
        }
    }
}

fn spawn_ball_system(cmd: &mut Commands, unowned_balls: Query<&BallId, Without<NetworkHandle>>) {
    let mut count = 0;
    let mut highest_id = 0;
    for ball in unowned_balls.iter() {
        count += 1;
        highest_id = highest_id.max(ball.0);
    }

    if count < 3 {
        cmd.spawn((
            BallId(highest_id + 1),
            Position(vec2(
                rand::random::<f32>() * 10.0 - 5.0,
                rand::random::<f32>() * 10.0 - 5.0,
            )),
            Velocity::default(),
            TargetVelocity::default(),
        ));

        println!("Spawned ball {:?}", highest_id + 1);
    }
}

fn unowned_ball_input_system(
    mut unowned_balls: Query<(&BallId, &mut TargetVelocity), Without<NetworkHandle>>,
) {
    for (_, mut target_velocity) in unowned_balls.iter_mut() {
        target_velocity.0.x = rand::random::<f32>() * 2.0 - 1.0;
        target_velocity.0.y = rand::random::<f32>() * 2.0 - 1.0;
    }
}

fn broadcast_changes_system(
    mut net: ResMut<NetworkResource>,
    changed_target_velocities: Query<(&BallId, &TargetVelocity), Changed<TargetVelocity>>,
    changed_positions: Query<(&BallId, &Position), Changed<Position>>,
) {
    for (ball_id, target_velocity) in changed_target_velocities.iter() {
        let _ = net.broadcast_message((*ball_id, *target_velocity));
    }

    for (ball_id, position) in changed_positions.iter() {
        let _ = net.broadcast_message((*ball_id, *position));
    }
}

fn read_component_channel_system<C: ChannelMessage>(
    cmd: &mut Commands,
    mut net: ResMut<NetworkResource>,
    balls_query: Query<(&BallId, Entity)>,
) {
    let balls: HashMap<BallId, Entity> = balls_query.iter().map(|(&b, e)| (b, e)).collect();

    for (_, connection) in net.connections.iter_mut() {
        let channels = connection.channels().unwrap();

        while let Some((ball_id, component)) = channels.recv::<(BallId, C)>() {
            match balls.get(&ball_id) {
                Some(&entity) => {
                    cmd.insert_one(entity, component);
                }
                None => (),
            }
        }
    }
}

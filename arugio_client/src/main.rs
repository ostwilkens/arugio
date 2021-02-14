use arugio_shared::{
    BallBundle, BallId, ClientMessage, Position, ServerMessage, TargetVelocity, Velocity,
};
use bevy::{math::vec3, prelude::*, render::camera::Camera};
use bevy_networking_turbulence::{NetworkEvent, NetworkResource, NetworkingPlugin};
use bevy_web_fullscreen::FullViewportPlugin;
use std::{
    collections::HashMap,
    f32::consts::PI,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};
use turbulence::message_channels::ChannelMessage;

struct LocalPlayer;

fn main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    console_log::init_with_level(log::Level::Debug).expect("cannot initialize console_log");

    App::build()
        .add_plugins(bevy_webgl2::DefaultPlugins)
        .add_plugin(NetworkingPlugin::default())
        .add_plugin(FullViewportPlugin)
        .add_resource(EventReader::<NetworkEvent>::default())
        .add_startup_system(arugio_shared::network_channels_setup.system())
        .add_startup_system(setup_world_system.system())
        .add_startup_system(client_setup_system.system())
        .add_system(add_ball_mesh_system.system())
        .add_system(handle_network_events_system.system())
        .add_system(keyboard_input_system.system())
        .add_system(handle_pointer_target_system.system())
        .add_system(arugio_shared::update_velocity_system.system())
        .add_system(arugio_shared::update_position_system.system())
        .add_system(update_ball_translation_system.system())
        .add_system(update_camera_translation_system.system())
        .add_system_to_stage(
            stage::PRE_UPDATE,
            read_component_channel_system::<Position>.system(),
        )
        .add_system_to_stage(
            stage::PRE_UPDATE,
            read_component_channel_system::<TargetVelocity>.system(),
        )
        .add_system_to_stage(
            stage::PRE_UPDATE,
            read_server_message_channel_system.system(),
        )
        .add_system_to_stage(stage::POST_UPDATE, broadcast_local_changes_system.system())
        .run();
}

fn setup_world_system(
    cmd: &mut Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let map_material = StandardMaterial {
        albedo: Color::rgb(0.15, 0.27, 0.33),
        albedo_texture: Some(asset_server.load("noise.png")),
        shaded: true,
    };

    cmd.spawn(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane { size: 200.0 })),
        material: materials.add(map_material),
        transform: Transform::from_rotation(Quat::from_rotation_x(PI * 0.5)),
        ..Default::default()
    })
    .spawn(Camera3dBundle {
        transform: Transform::from_translation(Vec3::new(0.0, 0.0, 15.0))
            .looking_at(Vec3::default(), Vec3::unit_y()),
        ..Default::default()
    })
    .with_children(|parent| {
        parent.spawn(LightBundle {
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, -10.0)),
            ..Default::default()
        });
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

fn read_component_channel_system<C: ChannelMessage>(
    cmd: &mut Commands,
    mut net: ResMut<NetworkResource>,
    balls_query: Query<(&BallId, Entity, Option<&LocalPlayer>)>,
) {
    let balls: HashMap<&BallId, (Entity, Option<&LocalPlayer>)> =
        balls_query.iter().map(|(b, e, l)| (b, (e, l))).collect();

    for (_, connection) in net.connections.iter_mut() {
        let channels = connection.channels().unwrap();

        while let Some((ball_id, component)) = channels.recv::<(BallId, C)>() {
            match balls.get(&ball_id) {
                Some((entity, local_player)) => {
                    if local_player.is_some() {
                        continue;
                    }
                    cmd.insert_one(*entity, component);
                }
                None => {
                    cmd.spawn(BallBundle::new(ball_id)).with(component);
                }
            }
        }
    }
}

fn read_server_message_channel_system(
    cmd: &mut Commands,
    mut net: ResMut<NetworkResource>,
    balls: Query<(Entity, &BallId)>,
) {
    for (_, connection) in net.connections.iter_mut() {
        let channels = connection.channels().unwrap();

        while let Some(message) = channels.recv::<ServerMessage>() {
            match message {
                ServerMessage::Welcome(your_ball_id) => {
                    let local_ball = balls
                        .iter()
                        .filter(|(_, &ball_id)| your_ball_id == ball_id)
                        .next();

                    match local_ball {
                        Some((entity, _)) => {
                            cmd.insert_one(entity, LocalPlayer);
                        }
                        None => {
                            cmd.spawn(BallBundle::new(your_ball_id)).with(LocalPlayer);
                        }
                    }
                }
            }
        }
    }
}

fn handle_network_events_system(
    mut net: ResMut<NetworkResource>,
    network_events: Res<Events<NetworkEvent>>,
    mut network_event_reader: ResMut<EventReader<NetworkEvent>>,
) {
    for event in network_event_reader.iter(&network_events) {
        match event {
            NetworkEvent::Connected(handle) => match net.connections.get_mut(handle) {
                Some(_connection) => {
                    info!("Connection successful");

                    net.send_message(*handle, ClientMessage::Hello)
                        .expect("Could not send hello");
                }
                None => panic!("Got packet for non-existing connection [{}]", handle),
            },
            _ => {}
        }
    }
}

fn handle_pointer_target_system(
    cmd: &mut Commands,
    windows: Res<Windows>,
    mouse_button_input: Res<Input<MouseButton>>,
    cursor_moved_events: Res<Events<CursorMoved>>,
    mut cursor_moved_event_reader: Local<EventReader<CursorMoved>>,
    local_players: Query<(Entity, &LocalPlayer, &TargetVelocity)>,
) {
    let local_player = local_players.iter().next();

    if let Some((player_entity, _, velocity)) = local_player {
        let mouse_down = mouse_button_input.pressed(MouseButton::Left);

        for event in cursor_moved_event_reader.iter(&cursor_moved_events) {
            if mouse_down {
                let window = windows.get_primary().unwrap();
                let resolution = Vec2::new(window.width() as f32, window.height() as f32);
                let screen_center = resolution / 2.0;
                let offset = event.position - screen_center;
                let power = 1.0 - (30.0 / offset.length()).min(1.0);
                let normal = offset.normalize();

                cmd.set_current_entity(player_entity);
                cmd.with(TargetVelocity(normal * power));
            }
        }

        if !mouse_down && velocity.0 != Vec2::zero() {
            cmd.set_current_entity(player_entity);
            cmd.with(TargetVelocity(Vec2::zero()));
        }
    }
}

fn update_ball_translation_system(mut balls: Query<(&Position, &mut Transform)>) {
    for (position, mut transform) in balls.iter_mut() {
        transform.translation.x = position.0.x;
        transform.translation.y = position.0.y;
        transform.rotation =
            Quat::from_rotation_ypr(position.0.x * PI / 2.0, -position.0.y * PI / 2.0, 0.0);
    }
}

fn update_camera_translation_system(
    local_players: Query<(&LocalPlayer, &Transform, &Velocity)>,
    mut cameras: Query<(&Camera, &mut Transform)>,
) {
    let local_player = local_players.iter().next();

    if let Some((_, local_player_transform, velocity)) = local_player {
        for (_, mut camera_transform) in cameras.iter_mut() {
            camera_transform.translation.x = local_player_transform.translation.x - velocity.0.x;
            camera_transform.translation.y = local_player_transform.translation.y - velocity.0.y;
            camera_transform.translation.z = 15.0 - velocity.0.length() * 4.0;
            let lookat = vec3(
                local_player_transform.translation.x + velocity.0.x,
                local_player_transform.translation.y + velocity.0.y,
                2.0,
            );
            camera_transform.look_at(lookat, Vec3::unit_y());
        }
    }
}

fn broadcast_local_changes_system(
    mut net: ResMut<NetworkResource>,
    changed_target_velocities: Query<
        (&LocalPlayer, &BallId, &TargetVelocity),
        Changed<TargetVelocity>,
    >,
    changed_positions: Query<(&LocalPlayer, &BallId, &Position), Changed<Position>>,
) {
    for (_, ball_id, target_velocity) in changed_target_velocities.iter() {
        let _ = net.broadcast_message((*ball_id, *target_velocity));
    }

    for (_, ball_id, position) in changed_positions.iter() {
        let _ = net.broadcast_message((*ball_id, *position));
    }
}

fn add_ball_mesh_system(
    cmd: &mut Commands,
    balls_without_mesh: Query<(Entity, &BallId), Without<Handle<Mesh>>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, _) in balls_without_mesh.iter() {
        cmd.insert(
            entity,
            PbrBundle {
                mesh: meshes.add(Mesh::from(shape::Icosphere {
                    radius: 0.5,
                    subdivisions: 0,
                })),
                material: materials.add(Color::rgb(0.91, 0.44, 0.32).into()),
                ..Default::default()
            },
        );
    }
}

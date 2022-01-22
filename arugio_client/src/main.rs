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

#[derive(Component)]
struct LocalPlayer;

fn main() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
    // console_log::init_with_level(log::Level::Debug).expect("cannot initialize console_log");

    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(NetworkingPlugin::default())
        .add_plugin(FullViewportPlugin)
        .add_startup_system(arugio_shared::setup_network_channels)
        .add_startup_system(setup_world)
        .add_startup_system(setup_client)
        .add_system(add_ball_mesh)
        .add_system(network_events)
        .add_system(keyboard_input)
        .add_system(pointer_target)
        .add_system(arugio_shared::update_velocity)
        .add_system(arugio_shared::update_position)
        .add_system(update_ball_translation)
        .add_system(update_camera_translation)
        .add_system_to_stage(CoreStage::PreUpdate, read_component_channel::<Position>)
        .add_system_to_stage(
            CoreStage::PreUpdate,
            read_component_channel::<TargetVelocity>,
        )
        .add_system_to_stage(CoreStage::PreUpdate, read_server_message_channel)
        .add_system_to_stage(CoreStage::PostUpdate, broadcast_local_changes)
        .run();
}

fn setup_world(
    mut cmd: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    asset_server: Res<AssetServer>,
) {
    let map_material = StandardMaterial {
        base_color: Color::rgb(0.15, 0.27, 0.33),
        base_color_texture: Some(asset_server.load("noise.png")),
        ..Default::default()
    };

    cmd.spawn_bundle(PbrBundle {
        mesh: meshes.add(Mesh::from(shape::Plane { size: 200.0 })),
        material: materials.add(map_material),
        transform: Transform::from_rotation(Quat::from_rotation_x(PI * 0.5)),
        ..Default::default()
    });

    cmd.spawn_bundle(PerspectiveCameraBundle {
        transform: Transform::from_translation(Vec3::new(0.0, 0.0, 15.0))
            .looking_at(Vec3::default(), Vec3::Y),
        ..Default::default()
    })
    .with_children(|parent| {
        parent.spawn_bundle(PointLightBundle {
            transform: Transform::from_translation(Vec3::new(0.0, 0.0, -10.0)),
            ..Default::default()
        });
    });
}

fn setup_client(mut net: ResMut<NetworkResource>) {
    let ip_address = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    let socket_address = SocketAddr::new(ip_address, 9001);
    info!("Connecting to {}...", socket_address);
    net.connect(socket_address);
}

fn keyboard_input(keyboard_input: Res<Input<KeyCode>>) {
    let pressed = keyboard_input.get_just_pressed();
    for key in pressed {
        info!("Keyboard input: {:?}", key);
    }
}

fn read_component_channel<C: Component + ChannelMessage>(
    mut cmd: Commands,
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

                    cmd.entity(*entity).insert(component);
                }
                None => {
                    cmd.spawn_bundle(BallBundle::new(ball_id)).insert(component);
                }
            }
        }
    }
}

fn read_server_message_channel(
    mut cmd: Commands,
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
                            cmd.entity(entity).insert(LocalPlayer);
                        }
                        None => {
                            cmd.spawn_bundle(BallBundle::new(your_ball_id))
                                .insert(LocalPlayer);
                        }
                    }
                }
            }
        }
    }
}

fn network_events(
    mut net: ResMut<NetworkResource>,
    mut network_event_reader: EventReader<NetworkEvent>,
) {
    for event in network_event_reader.iter() {
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

fn pointer_target(
    mut cmd: Commands,
    windows: Res<Windows>,
    mouse_button_input: Res<Input<MouseButton>>,
    mut cursor_moved_event_reader: EventReader<CursorMoved>,
    local_players: Query<(Entity, &LocalPlayer, &TargetVelocity)>,
) {
    let local_player = local_players.iter().next();

    if let Some((player_entity, _, velocity)) = local_player {
        let mouse_down = mouse_button_input.pressed(MouseButton::Left);

        for event in cursor_moved_event_reader.iter() {
            if mouse_down {
                let window = windows.get_primary().unwrap();
                let resolution = Vec2::new(window.width() as f32, window.height() as f32);
                let screen_center = resolution / 2.0;
                let offset = event.position - screen_center;
                let power = 1.0 - (30.0 / offset.length()).min(1.0);
                let normal = offset.normalize();

                cmd.entity(player_entity)
                    .insert(TargetVelocity(normal * power));
            }
        }

        if !mouse_down && velocity.0 != Vec2::ZERO {
            cmd.entity(player_entity).insert(TargetVelocity(Vec2::ZERO));
        }
    }
}

fn update_ball_translation(mut balls: Query<(&Position, &mut Transform)>) {
    for (position, mut transform) in balls.iter_mut() {
        transform.translation.x = position.0.x;
        transform.translation.y = position.0.y;
        transform.rotation = Quat::from_euler(
            EulerRot::YXZ,
            position.0.x * PI / 2.0,
            -position.0.y * PI / 2.0,
            0.0,
        );
    }
}

fn update_camera_translation(
    local_players: Query<(&LocalPlayer, &Transform, &Velocity)>,
    mut cameras: Query<(&Camera, &mut Transform, Without<LocalPlayer>)>,
) {
    let local_player = local_players.iter().next();

    if let Some((_, local_player_transform, velocity)) = local_player {
        let (_, mut camera_transform, _) = cameras.single_mut();

        camera_transform.translation.x = local_player_transform.translation.x - velocity.0.x;
        camera_transform.translation.y = local_player_transform.translation.y - velocity.0.y;
        camera_transform.translation.z = 15.0 - velocity.0.length() * 4.0;
        let lookat = vec3(
            local_player_transform.translation.x + velocity.0.x,
            local_player_transform.translation.y + velocity.0.y,
            2.0,
        );
        camera_transform.look_at(lookat, Vec3::Y);
    }
}

fn broadcast_local_changes(
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

fn add_ball_mesh(
    mut cmd: Commands,
    balls_without_mesh: Query<(Entity, &BallId), Without<Handle<Mesh>>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for (entity, _) in balls_without_mesh.iter() {
        cmd.entity(entity).insert_bundle(PbrBundle {
            mesh: meshes.add(Mesh::from(shape::Icosphere {
                radius: 0.5,
                subdivisions: 0,
            })),
            material: materials.add(Color::rgb(0.91, 0.44, 0.32).into()),
            ..Default::default()
        });
    }
}

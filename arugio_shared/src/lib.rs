use bevy::{
    core::Time,
    ecs::bundle::Bundle,
    prelude::{Query, Res},
};
use bevy::{math::Vec2, prelude::ResMut};
use bevy_networking_turbulence::{
    ConnectionChannelsBuilder, MessageChannelMode, MessageChannelSettings, NetworkResource,
    ReliableChannelSettings,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;

pub const CLIENT_MESSAGE_SETTINGS: MessageChannelSettings = MessageChannelSettings {
    channel: 0,
    channel_mode: MessageChannelMode::Reliable {
        reliability_settings: ReliableChannelSettings {
            bandwidth: 4096,
            recv_window_size: 1024,
            send_window_size: 1024,
            burst_bandwidth: 1024,
            init_send: 512,
            wakeup_time: Duration::from_millis(100),
            initial_rtt: Duration::from_millis(200),
            max_rtt: Duration::from_secs(2),
            rtt_update_factor: 0.1,
            rtt_resend_factor: 1.5,
        },
        max_message_len: 1024,
    },
    message_buffer_size: 8,
    packet_buffer_size: 8,
};

pub const SERVER_MESSAGE_SETTINGS: MessageChannelSettings = MessageChannelSettings {
    channel: 1,
    channel_mode: MessageChannelMode::Reliable {
        reliability_settings: ReliableChannelSettings {
            bandwidth: 4096,
            recv_window_size: 1024,
            send_window_size: 1024,
            burst_bandwidth: 1024,
            init_send: 512,
            wakeup_time: Duration::from_millis(100),
            initial_rtt: Duration::from_millis(200),
            max_rtt: Duration::from_secs(2),
            rtt_update_factor: 0.1,
            rtt_resend_factor: 1.5,
        },
        max_message_len: 1024,
    },
    message_buffer_size: 8,
    packet_buffer_size: 8,
};

fn player_component_message_settings(channel: u8) -> MessageChannelSettings {
    MessageChannelSettings {
        channel,
        channel_mode: MessageChannelMode::Unreliable,
        message_buffer_size: 8,
        packet_buffer_size: 8,
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ClientMessage {
    Hello,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ServerMessage {
    Welcome(BallId),
}

pub fn network_channels_setup(mut net: ResMut<NetworkResource>) {
    net.set_channels_builder(|builder: &mut ConnectionChannelsBuilder| {
        builder
            .register::<ClientMessage>(CLIENT_MESSAGE_SETTINGS)
            .unwrap();
        builder
            .register::<ServerMessage>(SERVER_MESSAGE_SETTINGS)
            .unwrap();
        builder
            .register::<(BallId, Position)>(player_component_message_settings(2))
            .unwrap();
        builder
            .register::<(BallId, Velocity)>(player_component_message_settings(3))
            .unwrap();
        builder
            .register::<(BallId, TargetVelocity)>(player_component_message_settings(4))
            .unwrap();
    });
}

#[derive(Serialize, Deserialize, Default, Clone, Copy, Debug, PartialEq, Hash, Eq)]
pub struct BallId(pub u32);
#[derive(Serialize, Deserialize, Default, Clone, Copy, Debug)]
pub struct Position(pub Vec2);
#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Velocity(pub Vec2);
#[derive(Serialize, Deserialize, Default, Clone, Copy, Debug)]
pub struct TargetVelocity(pub Vec2);

pub fn update_velocity_system(mut query: Query<(&mut Velocity, &TargetVelocity)>, time: Res<Time>) {
    let delta = time.delta_seconds();
    let speed = 2.0;

    for (mut velocity, target_velocity) in query.iter_mut() {
        velocity.0 = velocity.0 * (1.0 - delta * speed) + target_velocity.0 * (delta * speed);
    }
}

pub fn update_position_system(mut query: Query<(&mut Position, &Velocity)>, time: Res<Time>) {
    for (mut pos, vel) in query.iter_mut() {
        pos.0 += vel.0 * time.delta_seconds() * 15.0;
    }
}

#[derive(Bundle)]
pub struct BallBundle {
    ball_id: BallId,
    position: Position,
    velocity: Velocity,
    target_velocity: TargetVelocity,
}

impl BallBundle {
    pub fn new(ball_id: BallId) -> BallBundle {
        BallBundle {
            ball_id,
            position: Default::default(),
            velocity: Default::default(),
            target_velocity: Default::default(),
        }
    }
}

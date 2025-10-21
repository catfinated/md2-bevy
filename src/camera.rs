//! 3D camera control
//! This is based on the free cam example provided with bevy
//! but it's trimmed down to match more closely how the camera
//! in md2view works
use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use std::f32::consts::*;

#[derive(Component)]
pub struct CameraController {
    pub initialized: bool,
    pub mouse_sensitivity: f32,
    pub movement_speed: f32,
    pub friction: f32,
    pub pitch: f32,
    pub yaw: f32,
    pub velocity: Vec3,
}

impl Default for CameraController {
    fn default() -> Self {
        Self {
            initialized: false,
            mouse_sensitivity: 0.005,
            movement_speed: 3.0,
            friction: 0.5,
            pitch: 0.0,
            yaw: 0.0,
            velocity: Vec3::ZERO,
        }
    }
}

pub fn camera_control_system(
    time: Res<Time<Real>>,
    mut windows: Query<(&Window, &mut CursorOptions)>,
    accumulated_mouse_motion: Res<AccumulatedMouseMotion>,
    mouse_button_input: Res<ButtonInput<MouseButton>>,
    key_input: Res<ButtonInput<KeyCode>>,
    mut toggle_cursor_grab: Local<bool>,
    mut mouse_cursor_grab: Local<bool>,
    mut query: Query<(&mut Transform, &mut CameraController), With<Camera>>,
) {
    let Ok((mut transform, mut controller)) = query.single_mut() else {
        return;
    };

    if !controller.initialized {
        let (yaw, pitch, _roll) = transform.rotation.to_euler(EulerRot::YXZ);
        controller.yaw = yaw;
        controller.pitch = pitch;
        controller.initialized = true;
    }

    let mut axis_input = Vec3::ZERO;
    if key_input.pressed(KeyCode::KeyW) {
        axis_input.z += 1.0;
    }
    if key_input.pressed(KeyCode::KeyS) {
        axis_input.z -= 1.0;
    }
    if key_input.pressed(KeyCode::KeyD) {
        axis_input.x += 1.0;
    }
    if key_input.pressed(KeyCode::KeyA) {
        axis_input.x -= 1.0;
    }
    if key_input.pressed(KeyCode::KeyE) {
        axis_input.y += 1.0;
    }
    if key_input.pressed(KeyCode::KeyQ) {
        axis_input.y -= 1.0;
    }

    if axis_input != Vec3::ZERO {
        controller.velocity = axis_input.normalize() * controller.movement_speed;
    } else {
        let friction = controller.friction.clamp(0.0, 1.0);
        controller.velocity *= 1.0 - friction;
        if controller.velocity.length_squared() < 1e-6 {
            controller.velocity = Vec3::ZERO;
        }
    }

    if controller.velocity != Vec3::ZERO {
        let dt = time.delta_secs();
        let forward = *transform.forward();
        let right = *transform.right();
        transform.translation += controller.velocity.x * dt * right
            + controller.velocity.y * dt * Vec3::Y
            + controller.velocity.z * dt * forward;
    }

    let mouse_key_cursor_grab = MouseButton::Left;
    let mut cursor_grab_change = false;

    if key_input.just_pressed(KeyCode::KeyM) {
        *toggle_cursor_grab = !*toggle_cursor_grab;
        cursor_grab_change = true;
    }
    if mouse_button_input.just_pressed(mouse_key_cursor_grab) {
        *mouse_cursor_grab = true;
        cursor_grab_change = true;
    }
    if mouse_button_input.just_released(mouse_key_cursor_grab) {
        *mouse_cursor_grab = false;
        cursor_grab_change = true;
    }
    let cursor_grab = *mouse_cursor_grab || *toggle_cursor_grab;

    if cursor_grab_change {
        if cursor_grab {
            for (window, mut cursor_options) in &mut windows {
                if !window.focused {
                    continue;
                }

                cursor_options.grab_mode = CursorGrabMode::Locked;
                cursor_options.visible = false;
            }
        } else {
            for (_, mut cursor_options) in &mut windows {
                cursor_options.grab_mode = CursorGrabMode::None;
                cursor_options.visible = true;
            }
        }
    }

    // Handle mouse input
    if accumulated_mouse_motion.delta != Vec2::ZERO && cursor_grab {
        // Apply look update
        controller.pitch = (controller.pitch
            - accumulated_mouse_motion.delta.y * controller.mouse_sensitivity)
            .clamp(-PI / 2., PI / 2.);
        controller.yaw -= accumulated_mouse_motion.delta.x * controller.mouse_sensitivity;
        transform.rotation = Quat::from_euler(EulerRot::ZYX, 0.0, controller.yaw, controller.pitch);
    }
}

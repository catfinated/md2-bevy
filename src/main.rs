use bevy::{camera::visibility::RenderLayers, prelude::*};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext};
use md2_bevy::camera::{camera_control_system, CameraController};

use md2_bevy::md2::{find_md2, spawn_md2, MD2Component};
use rand::prelude::*;
use std::path::Path;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                camera_control_system,
                keyboard_input_system,
                animation_system,
            ),
        )
        .add_systems(EguiPrimaryContextPass, ui_system)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let all_md2 = find_md2(&Path::new("assets"));
    let md2_idx = rand::rng().random_range(0..all_md2.len());

    spawn_md2(
        &all_md2[md2_idx],
        &mut commands,
        &asset_server,
        &mut materials,
        &mut meshes,
    );

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_transform = Transform::from_xyz(0.0, 0.0, 3.0).looking_at(
        Vec3 {
            x: 0.0,
            y: 0.0,
            z: -1.0,
        },
        Vec3::Y,
    );

    // Camera in 3D space.
    commands.spawn((
        Camera3d::default(),
        camera_transform,
        CameraController::default(),
    ));

    commands.spawn((
        // The `PrimaryEguiContext` component requires everything needed to render a primary context.
        PrimaryEguiContext,
        Camera2d::default(),
        // Setting RenderLayers to none makes sure we won't render anything apart from the UI.
        RenderLayers::none(),
        Camera {
            order: 1,
            ..default()
        },
    ));
}

fn keyboard_input_system(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &mut MD2Component)>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyZ) {
        let (entity, mut md2) = query.single_mut().unwrap();
        let new_mat = md2.next_skin(&asset_server, &mut materials);
        commands.entity(entity).insert(new_mat);
    }

    if keyboard_input.just_pressed(KeyCode::KeyX) {
        let (_, mut md2) = query.single_mut().unwrap();
        md2.next_anim();
    }
}

fn animation_system(
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<(&Mesh3d, &mut MD2Component)>,
) {
    let (mesh, mut md2) = query.single_mut().unwrap();
    let vertices = md2.animate(time.delta_secs());
    let m = meshes.get_mut(mesh.id()).unwrap();
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
}

fn ui_system(
    mut contexts: EguiContexts,
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &mut MD2Component)>,
) -> Result {
    let (entity, mut md2) = query.single_mut()?;
    let mut curr_skin = md2.skin_idx;
    let mut curr_anim = md2.anim_idx;

    egui::Window::new("MD2").show(contexts.ctx_mut()?, |ui| {
        egui::ComboBox::from_label("skin")
            .selected_text(md2.skin_name())
            .show_ui(ui, |ui| {
                for (idx, skin) in md2.skins().iter().enumerate() {
                    ui.selectable_value(&mut curr_skin, idx, &skin.name);
                }
            });

        if curr_skin != md2.skin_idx {
            let new_mat = md2.set_skin_idx(curr_skin, &asset_server, &mut materials);
            commands.entity(entity).insert(new_mat);
        }

        egui::ComboBox::from_label("anim")
            .selected_text(md2.anim_name())
            .show_ui(ui, |ui| {
                for (idx, anim) in md2.animations().iter().enumerate() {
                    ui.selectable_value(&mut curr_anim, idx, &anim.name);
                }
            });

        if curr_anim != md2.anim_idx {
            md2.set_anim_idx(curr_anim);
        }
    });

    Ok(())
}

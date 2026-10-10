use bevy::{camera::visibility::RenderLayers, prelude::*};
use bevy_egui::{EguiContexts, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext, egui};
use md2_bevy::camera::{CameraController, camera_control_system};
use md2_bevy::md2::{MD2, MD2Component, MD2Resource, Md2Plugin, spawn_md2};
use md2_bevy::pcx::PcxLoaderPlugin;

#[derive(States, Debug, Clone, PartialEq, Eq, Hash, Default)]
enum AppState {
    #[default]
    PreLoad,
    Loading,
    Playing,
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(PcxLoaderPlugin)
        .add_plugins(Md2Plugin)
        .add_plugins(EguiPlugin::default())
        .init_state::<AppState>()
        .add_systems(OnEnter(AppState::PreLoad), preload)
        .add_systems(OnEnter(AppState::Loading), load_assets)
        .add_systems(Update, check_loading.run_if(in_state(AppState::Loading)))
        .add_systems(OnEnter(AppState::Playing), setup_game)
        .add_systems(
            Update,
            (
                camera_control_system,
                keyboard_input_system,
                animation_system,
            )
                .run_if(in_state(AppState::Playing)),
        )
        .add_systems(
            EguiPrimaryContextPass,
            ui_system.run_if(in_state(AppState::Playing)),
        )
        .run();
}

fn preload(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    all_md2: Res<MD2Resource>,
    mut exit: MessageWriter<AppExit>,
) {
    if all_md2.fpaths.is_empty() {
        error!(
            "No .md2 models found under the assets directory. Extract them from your Quake 2 \
             PAK files into assets/ (see README)."
        );
        exit.write(AppExit::error());
        return;
    }
    next_state.set(AppState::Loading);
    commands.spawn((
        // The `PrimaryEguiContext` component requires everything needed to render a primary context.
        PrimaryEguiContext,
        Camera2d,
        // Setting RenderLayers to none makes sure we won't render anything apart from the UI.
        RenderLayers::none(),
        Camera {
            order: 1,
            ..default()
        },
    ));
}

fn load_assets(mut dpak: ResMut<MD2Resource>, asset_server: Res<AssetServer>) {
    let mut assets = Vec::new();
    for path in &dpak.fpaths {
        assets.push(asset_server.load::<MD2>(path.clone()));
    }
    dpak.assets = assets;
}

fn check_loading(
    mut next_state: ResMut<NextState<AppState>>,
    asset_server: Res<AssetServer>,
    all_md2: Res<MD2Resource>,
) {
    let all_loaded = all_md2.assets.iter().all(|handle| {
        matches!(
            asset_server.load_state(handle.id()),
            bevy::asset::LoadState::Loaded
        )
    });

    if all_loaded {
        next_state.set(AppState::Playing);
    }
}

fn setup_game(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    md2s: Res<Assets<MD2>>,
    all_md2: Res<MD2Resource>,
) {
    println!("setup game");
    let md2_handle = all_md2.assets[all_md2.curr_idx].clone();
    spawn_md2(&mut commands, &mut meshes, &md2_handle, &md2s);

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
}

fn keyboard_input_system(
    mut commands: Commands,
    mut query: Query<(Entity, &mut MD2Component)>,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    md2s: Res<Assets<MD2>>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyZ) {
        let (entity, mut md2c) = query.single_mut().unwrap();
        let md2 = md2s.get(&md2c.handle).unwrap();
        let next = md2.next_skin(md2c.skin_idx);
        md2c.set_skin_idx(next);
        let new_mat = MeshMaterial3d(md2.get_skin(next).clone());
        commands.entity(entity).insert(new_mat);
    }

    if keyboard_input.just_pressed(KeyCode::KeyX) {
        let (_, mut md2c) = query.single_mut().unwrap();
        let md2 = md2s.get(&md2c.handle).unwrap();
        let next = md2.next_anim(md2c.anim_idx);
        md2c.set_anim_idx(next);
    }
}

fn animation_system(
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<(&Mesh3d, &mut MD2Component)>,
    time: Res<Time>,
    md2s: Res<Assets<MD2>>,
) {
    let (mesh, mut md2c) = query.single_mut().unwrap();
    let md2 = md2s.get(&md2c.handle).unwrap();
    let vertices = md2c.animate(time.delta_secs(), md2);
    let m = meshes.get_mut(mesh.id()).unwrap();
    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, vertices);
}

fn ui_system(
    mut contexts: EguiContexts,
    mut commands: Commands,
    mut all_md2: ResMut<MD2Resource>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<(Entity, &mut MD2Component)>,
    md2s: Res<Assets<MD2>>,
) -> Result {
    let (entity, mut md2c) = query.single_mut()?;
    let mut curr_skin = md2c.skin_idx;
    let mut curr_anim = md2c.anim_idx;
    let mut curr_md2 = all_md2.curr_idx;
    let handle = &all_md2.assets[all_md2.curr_idx];
    let md2 = md2s.get(handle).unwrap();

    egui::Window::new("MD2").show(contexts.ctx_mut()?, |ui| {
        egui::ComboBox::from_label("md2")
            .selected_text(all_md2.curr_name())
            .show_ui(ui, |ui| {
                for (idx, name) in all_md2.names.iter().enumerate() {
                    ui.selectable_value(&mut curr_md2, idx, name);
                }
            });

        if curr_md2 != all_md2.curr_idx {
            all_md2.curr_idx = curr_md2;
            commands.entity(entity).despawn();
            spawn_md2(&mut commands, &mut meshes, &all_md2.assets[curr_md2], &md2s);
        }

        egui::ComboBox::from_label("skin")
            .selected_text(md2.skin_name(curr_skin))
            .show_ui(ui, |ui| {
                for (idx, skin) in md2.skins().iter().enumerate() {
                    ui.selectable_value(&mut curr_skin, idx, &skin.name);
                }
            });

        if curr_skin != md2c.skin_idx {
            md2c.set_skin_idx(curr_skin);
            commands
                .entity(entity)
                .insert(MeshMaterial3d(md2.get_skin(curr_skin).clone()));
        }

        egui::ComboBox::from_label("anim")
            .selected_text(md2.anim_name(curr_anim))
            .show_ui(ui, |ui| {
                for (idx, anim) in md2.animations().iter().enumerate() {
                    ui.selectable_value(&mut curr_anim, idx, &anim.name);
                }
            });

        if curr_anim != md2c.anim_idx {
            md2c.set_anim_idx(curr_anim);
        }
    });

    Ok(())
}

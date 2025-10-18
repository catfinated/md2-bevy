use bevy::{
    asset::{AssetPath, RenderAssetUsages},
    camera::visibility::RenderLayers,
    prelude::*,
    render::render_resource::PrimitiveTopology,
};
use bevy_egui::{egui, EguiContexts, EguiPlugin, EguiPrimaryContextPass, PrimaryEguiContext};

use md2_bevy::md2::MD2;
use rand::prelude::*;
use std::path::Path;

#[derive(Component)]
struct MD2Component {
    md2: MD2,
    skin_idx: usize,
    anim_idx: usize,
    curr_frame: usize,
    interp: f32,
    materials: Vec<Option<Handle<StandardMaterial>>>,
}

impl MD2Component {
    fn new(md2: MD2) -> Self {
        let skin_idx = rand::rng().random_range(0..md2.skins.len());
        let anim_idx = rand::rng().random_range(0..md2.animations.len());
        let materials: Vec<Option<Handle<StandardMaterial>>> = vec![None; md2.skins.len()];

        Self {
            md2,
            skin_idx,
            anim_idx,
            curr_frame: 0,
            interp: 0.0,
            materials,
        }
    }

    fn skin_name(&self) -> &str {
        &self.md2.skins[self.skin_idx].name
    }

    fn num_anim_frames(&self) -> usize {
        self.md2.animations[self.anim_idx].key_frames.len()
    }

    fn anim_name(&self) -> &str {
        &self.md2.animations[self.anim_idx].name
    }

    fn set_skin_idx(
        &mut self,
        idx: usize,
        asset_server: &Res<AssetServer>,
        materials: &mut ResMut<Assets<StandardMaterial>>,
    ) -> MeshMaterial3d<StandardMaterial> {
        self.skin_idx = idx;

        if self.materials[idx].is_none() {
            let path = AssetPath::from_path_buf(self.md2.skins[idx].path.clone());
            let texture_handle: Handle<Image> = asset_server.load(path);
            let mat_handle: Handle<StandardMaterial> = materials.add(StandardMaterial {
                base_color_texture: Some(texture_handle),
                unlit: true,
                ..default()
            });

            self.materials[idx] = Some(mat_handle);
        }

        MeshMaterial3d(self.materials[idx].as_ref().unwrap().clone())
    }

    fn set_anim_idx(&mut self, idx: usize) {
        self.anim_idx = idx;
        self.curr_frame = 0;
        self.interp = 0.0;
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(EguiPlugin::default())
        .add_systems(Startup, setup)
        .add_systems(Update, keyboard_input_system)
        .add_systems(Update, animation_system)
        .add_systems(EguiPrimaryContextPass, ui_example_system)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let md2 = MD2::load(&Path::new("assets/models/ogro/tris.md2"));
    let mut component = MD2Component::new(md2);
    let mat3d = component.set_skin_idx(component.skin_idx, &asset_server, &mut materials);
    let mesh_handle: Handle<Mesh> = meshes.add(create_mesh(&component));
    let scale = 1.0_f32 / 32.0_f32;
    let neg90 = f32::to_radians(-90.0);

    commands.spawn((
        Mesh3d(mesh_handle),
        mat3d,
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZEx, 0.0, neg90, 0.0))
            .with_scale(Vec3::splat(scale)),
        component,
    ));

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_transform = Transform::from_xyz(1.0, 1.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Camera in 3D space.
    commands.spawn((Camera3d::default(), camera_transform));

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

fn create_mesh(md2c: &MD2Component) -> Mesh {
    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(
        Mesh::ATTRIBUTE_POSITION,
        md2c.md2.animations[md2c.anim_idx].key_frames[0].clone(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, md2c.md2.texcoords.clone())
}

fn keyboard_input_system(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &mut MD2Component)>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyS) {
        let (entity, mut md2c) = query.single_mut().unwrap();
        let new_idx = (md2c.skin_idx + 1) % md2c.md2.skins.len();
        let new_mat = md2c.set_skin_idx(new_idx, &asset_server, &mut materials);
        commands.entity(entity).insert(new_mat);
    }

    if keyboard_input.just_pressed(KeyCode::KeyA) {
        let (_, mut md2c) = query.single_mut().unwrap();
        let next = (md2c.anim_idx + 1) % md2c.md2.animations.len();
        md2c.set_anim_idx(next);
    }
}

fn animation_system(
    time: Res<Time>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut query: Query<(&Mesh3d, &mut MD2Component)>,
) {
    let (mesh, mut md2c) = query.single_mut().unwrap();
    let mut interp = md2c.interp + (8.0f32 * time.delta_secs());
    let mut current = md2c.curr_frame;
    let mut next = (current + 1) % md2c.num_anim_frames();

    if interp >= 1.0f32 {
        current = next;
        next = (current + 1) % md2c.num_anim_frames();
        interp = 0.0f32;
    }
    md2c.interp = interp;
    md2c.curr_frame = current;

    let curr_v = &md2c.md2.animations[md2c.anim_idx].key_frames[current];
    let next_v = &md2c.md2.animations[md2c.anim_idx].key_frames[next];
    let mut v = Vec::new();
    v.reserve(curr_v.len());

    for i in 0..curr_v.len() {
        v.push(curr_v[i].lerp(next_v[i], interp));
    }

    let m = meshes.get_mut(mesh.id()).unwrap();

    m.insert_attribute(Mesh::ATTRIBUTE_POSITION, v);
}

fn ui_example_system(
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
        egui::ComboBox::from_label("[s]kin")
            .selected_text(md2.skin_name())
            .show_ui(ui, |ui| {
                for (idx, skin) in md2.md2.skins.iter().enumerate() {
                    ui.selectable_value(&mut curr_skin, idx, &skin.name);
                }
            });

        if curr_skin != md2.skin_idx {
            let new_mat = md2.set_skin_idx(curr_skin, &asset_server, &mut materials);
            commands.entity(entity).insert(new_mat);
        }

        egui::ComboBox::from_label("[a]nim")
            .selected_text(md2.anim_name())
            .show_ui(ui, |ui| {
                for (idx, anim) in md2.md2.animations.iter().enumerate() {
                    ui.selectable_value(&mut curr_anim, idx, &anim.name);
                }
            });

        if curr_anim != md2.anim_idx {
            md2.set_anim_idx(curr_anim);
        }
    });

    Ok(())
}

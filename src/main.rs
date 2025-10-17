use bevy::{
    asset::{AssetPath, RenderAssetUsages},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};
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
}

impl MD2Component {
    fn new(md2: MD2) -> Self {
        let skin_idx = rand::rng().random_range(0..md2.skins.len());
        let anim_idx = rand::rng().random_range(0..md2.animations.len());

        Self {
            md2,
            skin_idx,
            anim_idx,
            curr_frame: 0,
            interp: 0.0,
        }
    }

    fn skin_name(&self) -> &str {
        self.md2.skins[self.skin_idx]
            .file_stem()
            .unwrap()
            .to_str()
            .unwrap()
    }

    fn skin_path(&self) -> AssetPath<'_> {
        AssetPath::from_path_buf(self.md2.skins[self.skin_idx].clone())
    }

    fn num_anim_frames(&self) -> usize {
        self.md2.animations[self.anim_idx].key_frames.len()
    }

    fn anim_name(&self) -> &str {
        &self.md2.animations[self.anim_idx].name
    }
}

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, setup)
        .add_systems(Update, keyboard_input_system)
        .add_systems(Update, animation_system)
        .run();
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let md2 = MD2::load(&Path::new("assets/models/ogro/tris.md2"));
    //let md2 = MD2::load(&Path::new("assets/models/drfreak/tris.md2"));
    let component = MD2Component::new(md2);
    let texture_handle: Handle<Image> = asset_server.load(component.skin_path());
    let mesh_handle: Handle<Mesh> = meshes.add(create_mesh(&component));
    let scale = 1.0_f32 / 32.0_f32;
    let neg90 = f32::to_radians(-90.0);

    commands.spawn(Text::new(format_help(&component)));

    commands.spawn((
        Mesh3d(mesh_handle),
        MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(texture_handle),
            unlit: true,
            ..default()
        })),
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZEx, 0.0, neg90, 0.0))
            .with_scale(Vec3::splat(scale)),
        component,
    ));

    // Transform for the camera and lighting, looking at (0,0,0) (the position of the mesh).
    let camera_transform = Transform::from_xyz(1.0, 1.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Camera in 3D space.
    commands.spawn((Camera3d::default(), camera_transform));

    let light_transform = Transform::from_xyz(1.0, 1.0, 5.0).looking_at(Vec3::ZERO, Vec3::Y);

    // Light up the scene.
    commands.spawn((PointLight::default(), light_transform));
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

fn format_help(md2c: &MD2Component) -> String {
    format!("[s]kin: {}\n[a]nim: {}", md2c.skin_name(), md2c.anim_name())
}

fn keyboard_input_system(
    mut commands: Commands,
    keyboard_input: Res<ButtonInput<KeyCode>>,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut query: Query<(Entity, &mut MD2Component)>,
    query2: Query<(Entity, &Text)>,
) {
    if keyboard_input.just_pressed(KeyCode::KeyS) {
        let (entity, mut md2c) = query.single_mut().unwrap();
        let new_idx = (md2c.skin_idx + 1) % md2c.md2.skins.len();
        let path = AssetPath::from_path_buf(md2c.md2.skins[new_idx].clone());
        let texture_handle: Handle<Image> = asset_server.load(path);
        let new_mat = MeshMaterial3d(materials.add(StandardMaterial {
            base_color_texture: Some(texture_handle),
            unlit: true,
            ..default()
        }));
        md2c.skin_idx = new_idx;
        commands.entity(entity).insert(new_mat);

        let (e, _) = query2.single().unwrap();
        commands.entity(e).insert(Text::new(format_help(&md2c)));
    }

    if keyboard_input.just_pressed(KeyCode::KeyA) {
        let (_, mut md2c) = query.single_mut().unwrap();
        let next = (md2c.anim_idx + 1) % md2c.md2.animations.len();
        md2c.anim_idx = next;
        md2c.curr_frame = 0;
        md2c.interp = 0.0;

        let (e, _) = query2.single().unwrap();
        commands.entity(e).insert(Text::new(format_help(&md2c)));
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

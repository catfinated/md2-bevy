//! MD2 file loading and compenent
use bevy::reflect::TypePath;
use bevy::{
    asset::{Asset, AssetLoader, AssetPath, LoadContext, RenderAssetUsages, io::Reader},
    prelude::*,
    render::render_resource::PrimitiveTopology,
};
use glob::glob;
use rand::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Md2LoaderError {
    #[error("Failed to read MD2 file: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid MD2 format: {0}")]
    InvalidFormat(String),
}

/// MD2 file header
#[derive(Debug)]
#[repr(C)]
struct Header {
    ident: i32,
    version: i32,
    skinwidth: i32,
    skinheight: i32,
    framesize: i32,
    num_skins: i32,
    num_xyz: i32,
    num_st: i32,
    num_tris: i32,
    num_glcmds: i32,
    num_frames: i32,
    offset_skins: i32,
    offset_st: i32,
    offset_tris: i32,
    offset_frames: i32,
    offset_glcmd: i32,
    offset_end: i32,
}

impl Header {
    fn from_bytes(data: &[u8]) -> Result<Header, Md2LoaderError> {
        assert!(std::mem::size_of::<Header>() == 68);

        if data.len() < std::mem::size_of::<Header>() {
            return Err(Md2LoaderError::InvalidFormat(
                "Not enough bytes for header".to_string(),
            ));
        }

        let hdr_bytes: [u8; std::mem::size_of::<Header>()] =
            data[0..std::mem::size_of::<Header>()].try_into().unwrap();
        // TODO: remove unsafe block
        let header: Header = unsafe { std::mem::transmute(hdr_bytes) };
        Ok(header)
    }
}

/// Scaled texture coordinates
#[derive(Debug)]
#[repr(C)]
struct TexCoord {
    s: i16,
    t: i16,
}

impl TexCoord {
    fn from_bytes(data: &[u8]) -> Result<TexCoord, Md2LoaderError> {
        if data.len() < std::mem::size_of::<TexCoord>() {
            return Err(Md2LoaderError::InvalidFormat(
                "Not enough bytes for texcoord".to_string(),
            ));
        }

        let s = i16::from_le_bytes([data[0], data[1]]);
        let t = i16::from_le_bytes([data[2], data[3]]);
        Ok(TexCoord { s, t })
    }
}

/// MD2 Indexed triangle
#[derive(Debug)]
#[repr(C)]
struct Triangle {
    vertex: [u16; 3],
    st: [u16; 3],
}

impl Triangle {
    fn from_bytes(data: &[u8]) -> Result<Triangle, Md2LoaderError> {
        if data.len() < std::mem::size_of::<Triangle>() {
            return Err(Md2LoaderError::InvalidFormat(
                "Not enough bytes for triangle".to_string(),
            ));
        }

        let vertex = [
            u16::from_le_bytes(data[0..2].try_into().unwrap()),
            u16::from_le_bytes(data[2..4].try_into().unwrap()),
            u16::from_le_bytes(data[4..6].try_into().unwrap()),
        ];

        let st = [
            u16::from_le_bytes(data[6..8].try_into().unwrap()),
            u16::from_le_bytes(data[8..10].try_into().unwrap()),
            u16::from_le_bytes(data[10..12].try_into().unwrap()),
        ];

        Ok(Triangle { vertex, st })
    }
}

/// MD2 Scaled 3d vertex
#[derive(Debug)]
#[repr(C)]
struct Vertex {
    v: [u8; 3],
    normal_index: u8,
}

impl Vertex {
    fn from_bytes(data: &[u8]) -> Result<Vertex, Md2LoaderError> {
        if data.len() < std::mem::size_of::<Vertex>() {
            return Err(Md2LoaderError::InvalidFormat(
                "Not enough bytes for vertex".to_string(),
            ));
        }

        let v: [u8; 3] = data[0..3].try_into().unwrap();
        let normal_index = data[3];
        Ok(Vertex { v, normal_index })
    }
}

/// MD2 Animation key frame
#[derive(Debug)]
#[repr(C)]
struct Frame {
    scale: [f32; 3],
    translate: [f32; 3],
    name: [u8; 16],
}

impl Frame {
    fn from_bytes(data: &[u8]) -> Result<Frame, Md2LoaderError> {
        if data.len() < std::mem::size_of::<Frame>() {
            return Err(Md2LoaderError::InvalidFormat(
                "Not enough bytes for frame".to_string(),
            ));
        }

        let scale = [
            f32::from_le_bytes(data[0..4].try_into().unwrap()),
            f32::from_le_bytes(data[4..8].try_into().unwrap()),
            f32::from_le_bytes(data[8..12].try_into().unwrap()),
        ];

        let translate = [
            f32::from_le_bytes(data[12..16].try_into().unwrap()),
            f32::from_le_bytes(data[16..20].try_into().unwrap()),
            f32::from_le_bytes(data[20..24].try_into().unwrap()),
        ];

        let name: [u8; 16] = data[24..40].try_into().unwrap();

        Ok(Frame {
            scale,
            translate,
            name,
        })
    }

    fn get_name(&self) -> String {
        let s = String::from_utf8_lossy(&self.name);
        let mut end = s.len();
        if let Some(index) = s.find(|c: char| c.is_ascii_digit() || c.is_ascii_control()) {
            end = index;
        }

        s[0..end].to_string()
    }
}

type KeyFrame = Vec<Vec3>;

/// Decompressed animation key frame
///
/// For simplicity, this directly stores all the
/// 3d vertices per frame in the animation.
#[derive(Debug)]
pub struct Animation {
    pub name: String,
    pub key_frames: Vec<KeyFrame>,
}

/// On-disk skin data
#[derive(Debug)]
pub struct Skin {
    pub name: String,
    pub path: PathBuf,
}

/// MD2 model
#[derive(Asset, Debug, TypePath)]
pub struct MD2 {
    pub animations: Vec<Animation>,
    texcoords: Vec<Vec2>,
    pub skins: Vec<Skin>,
    materials: Vec<Handle<StandardMaterial>>,
}

impl MD2 {
    fn from_bytes(data: &[u8], fpath: &Path) -> Result<MD2, Md2LoaderError> {
        let header = Header::from_bytes(data)?;
        let triangles = MD2::load_triangles(data, &header)?;
        let texcoords = MD2::load_texcoords(data, &header, &triangles)?;
        let animations = MD2::load_animations(data, &header, &triangles)?;
        let skins = MD2::find_skins(fpath); // skins - only from directory right now
        let materials: Vec<Handle<StandardMaterial>> = Vec::with_capacity(skins.len());

        Ok(MD2 {
            animations,
            texcoords,
            skins,
            materials,
        })
    }

    fn load_triangles(data: &[u8], header: &Header) -> Result<Vec<Triangle>, Md2LoaderError> {
        let num_tris = usize::try_from(header.num_tris).map_err(|err| {
            Md2LoaderError::InvalidFormat(format!("Invalid number of triangles - {}", err))
        })?;
        let tris_off = usize::try_from(header.offset_tris).map_err(|err| {
            Md2LoaderError::InvalidFormat(format!("Invalid triangles offset - {}", err))
        })?;

        let mut triangles = Vec::with_capacity(num_tris);

        for i in 0..header.num_tris {
            let off = tris_off + (i as usize * std::mem::size_of::<Triangle>());
            let triangle = Triangle::from_bytes(&data[off..])?;
            triangles.push(triangle);
        }

        Ok(triangles)
    }

    fn load_texcoords(
        data: &[u8],
        header: &Header,
        triangles: &Vec<Triangle>,
    ) -> Result<Vec<Vec2>, Md2LoaderError> {
        let num_st = usize::try_from(header.num_st).map_err(|err| {
            Md2LoaderError::InvalidFormat(format!("Invalid number of texcoords - {}", err))
        })?;
        let st_off = usize::try_from(header.offset_st).map_err(|err| {
            Md2LoaderError::InvalidFormat(format!("Invalid texcoords offset - {}", err))
        })?;

        let mut unscaled_texcoords = Vec::with_capacity(num_st);

        for i in 0..num_st {
            let off = st_off + (i * std::mem::size_of::<TexCoord>());
            let texcoord = TexCoord::from_bytes(&data[off..])?;
            unscaled_texcoords.push(texcoord);
        }

        let skin_width = header.skinwidth as f32;
        let skin_height = header.skinheight as f32;

        let mut texcoords = Vec::with_capacity(triangles.len() * 3);

        for tri in triangles {
            for i in 0..3 {
                let index = usize::from(tri.st[i]);
                let texcoord = &unscaled_texcoords[index];
                let s = f32::from(texcoord.s) / skin_width;
                let t = f32::from(texcoord.t) / skin_height;
                texcoords.push(Vec2::new(s, t));
            }
        }

        Ok(texcoords)
    }

    fn read_and_decompress_vertices(
        data: &[u8],
        num_xyz: usize,
        frame: &Frame,
        triangles: &Vec<Triangle>,
    ) -> Result<Vec<Vec3>, Md2LoaderError> {
        let mut raw_vertices: Vec<Vertex> = Vec::with_capacity(num_xyz);

        for i in 0..num_xyz {
            let off = i * std::mem::size_of::<Vertex>();
            let vertex = Vertex::from_bytes(&data[off..])?;
            raw_vertices.push(vertex);
        }

        let mut vertices = Vec::with_capacity(triangles.len() * 3);

        for tri in triangles {
            for i in 0..3 {
                let vi = usize::from(tri.vertex[i]);
                let vertex = &raw_vertices[vi];
                // NB: pay attention to the assingments here as we swap z and y
                let x = (frame.scale[0] * vertex.v[0] as f32) + frame.translate[0];
                let z = (frame.scale[1] * vertex.v[1] as f32) + frame.translate[1];
                let y = (frame.scale[2] * vertex.v[2] as f32) + frame.translate[2];
                vertices.push(Vec3::new(x, y, z));
            }
        }

        Ok(vertices)
    }

    fn load_animations(
        data: &[u8],
        header: &Header,
        triangles: &Vec<Triangle>,
    ) -> Result<Vec<Animation>, Md2LoaderError> {
        let num_xyz = usize::try_from(header.num_xyz).map_err(|err| {
            Md2LoaderError::InvalidFormat(format!("Invalid number of vertices - {}", err))
        })?;
        let frames_off = usize::try_from(header.offset_frames).map_err(|err| {
            Md2LoaderError::InvalidFormat(format!("Invalid frames offset - {}", err))
        })?;

        let mut key_frames: Vec<KeyFrame> = Vec::new();
        let mut animations: Vec<Animation> = Vec::new();
        let mut last_frame_name: Option<String> = None;
        let mut off = frames_off;

        for _ in 0..header.num_frames {
            let frame = Frame::from_bytes(&data[off..])?;
            off += std::mem::size_of::<Frame>();
            let vertices =
                MD2::read_and_decompress_vertices(&data[off..], num_xyz, &frame, triangles)?;
            off += num_xyz * std::mem::size_of::<Vertex>();

            let curr_name = frame.get_name();
            if let Some(prev_name) = last_frame_name
                && prev_name != curr_name
            {
                animations.push(Animation {
                    name: prev_name.clone(),
                    key_frames,
                });

                key_frames = Vec::new();
            }
            last_frame_name = Some(curr_name);

            key_frames.push(vertices);
        }

        if !key_frames.is_empty() {
            animations.push(Animation {
                name: last_frame_name.unwrap(),
                key_frames,
            });
        }

        Ok(animations)
    }

    fn find_skins(fpath: &Path) -> Vec<Skin> {
        let extensions = ["*.pcx", "*.png"];
        let mut skins = HashMap::new();
        let assets = Path::new("assets");

        for ext in extensions {
            let glob_path = assets.join(fpath.parent().unwrap().join(ext));
            let pattern = glob_path.to_str().unwrap();

            for entry in glob(pattern).unwrap().filter_map(Result::ok) {
                let path = entry.strip_prefix("assets").unwrap().to_path_buf();
                let name = path.file_stem().unwrap().to_str().unwrap().to_string();

                skins.entry(name).or_insert(path);
            }
        }

        skins
            .iter()
            .map(|(k, v)| Skin {
                name: k.clone(),
                path: v.clone(),
            })
            .collect()
    }

    // Skins
    pub fn skins(&self) -> &[Skin] {
        &self.skins
    }

    pub fn skin_name(&self, skin_idx: usize) -> &str {
        &self.skins[skin_idx].name
    }

    pub fn next_skin(&self, skin_idx: usize) -> usize {
        (skin_idx + 1) % self.skins.len()
    }

    pub fn get_skin(&self, skin_idx: usize) -> &Handle<StandardMaterial> {
        &self.materials[skin_idx]
    }

    // Animations
    pub fn animations(&self) -> &[Animation] {
        &self.animations
    }

    fn num_anim_frames(&self, anim_idx: usize) -> usize {
        self.animations[anim_idx].key_frames.len()
    }

    pub fn anim_name(&self, anim_idx: usize) -> &str {
        &self.animations[anim_idx].name
    }

    pub fn next_anim(&self, anim_idx: usize) -> usize {
        (anim_idx + 1) % self.animations.len()
    }
}

#[derive(Default, TypePath)]
pub struct Md2Loader;

impl AssetLoader for Md2Loader {
    type Asset = MD2;
    type Settings = ();
    type Error = Md2LoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;

        let mut md2 = MD2::from_bytes(&bytes, load_context.path().path())?;

        for skin in &md2.skins {
            let path = AssetPath::from_path_buf(skin.path.clone());
            let texture = load_context.load(path);
            let material = StandardMaterial {
                base_color_texture: Some(texture),
                unlit: true,
                ..default()
            };
            let handle = load_context.add_labeled_asset(skin.name.clone(), material);
            md2.materials.push(handle);
        }

        Ok(md2)
    }

    fn extensions(&self) -> &[&str] {
        &["md2"]
    }
}

/// MD2 Bevy Component
///
/// Allows changing the current animation and skin.
#[derive(Component)]
pub struct MD2Component {
    pub skin_idx: usize,
    pub anim_idx: usize,
    curr_frame: usize,
    interp: f32,
    pub handle: Handle<MD2>,
}

impl MD2Component {
    pub fn set_skin_idx(&mut self, idx: usize) {
        self.skin_idx = idx;
    }

    pub fn set_anim_idx(&mut self, idx: usize) {
        self.anim_idx = idx;
        self.curr_frame = 0;
        self.interp = 0.0;
    }

    pub fn animate(&mut self, delta: f32, md2: &MD2) -> Vec<Vec3> {
        let mut interp = self.interp + (8.0f32 * delta);
        let mut current = self.curr_frame;
        let mut next = (current + 1) % md2.num_anim_frames(self.anim_idx);

        if interp >= 1.0f32 {
            current = next;
            next = (current + 1) % md2.num_anim_frames(self.anim_idx);
            interp = 0.0f32;
        }
        self.interp = interp;
        self.curr_frame = current;

        let curr_v = &md2.animations[self.anim_idx].key_frames[current];
        let next_v = &md2.animations[self.anim_idx].key_frames[next];
        let mut v = Vec::with_capacity(curr_v.len());

        for i in 0..curr_v.len() {
            v.push(curr_v[i].lerp(next_v[i], interp));
        }

        v
    }

    fn create_mesh(&self, md2: &MD2) -> Mesh {
        Mesh::new(
            PrimitiveTopology::TriangleList,
            RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
        )
        .with_inserted_attribute(
            Mesh::ATTRIBUTE_POSITION,
            md2.animations[self.anim_idx].key_frames[self.curr_frame].clone(),
        )
        .with_inserted_attribute(Mesh::ATTRIBUTE_UV_0, md2.texcoords.clone())
    }
}

/// Resource for available MD2 models
///
/// Tracks which model is currently selected.
#[derive(Resource)]
pub struct MD2Resource {
    pub fpaths: Vec<PathBuf>,
    pub names: Vec<String>,
    pub curr_idx: usize,
    pub assets: Vec<Handle<MD2>>,
}

impl MD2Resource {
    pub fn load(dpath: &Path) -> Self {
        let fpaths = find_md2(dpath);
        let names = fpaths
            .iter()
            .map(|p| MD2Resource::get_model_name(p.as_path()))
            .collect();
        let curr_idx = if fpaths.is_empty() {
            0
        } else {
            rand::rng().random_range(0..fpaths.len())
        };
        let assets = Vec::new();

        MD2Resource {
            fpaths,
            names,
            curr_idx,
            assets,
        }
    }

    pub fn curr_path(&self) -> &Path {
        &self.fpaths[self.curr_idx]
    }

    pub fn curr_name(&self) -> &str {
        &self.names[self.curr_idx]
    }

    fn get_model_name(fpath: &Path) -> String {
        let model = fpath
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();
        model.to_string()
    }
}

pub struct Md2Plugin;

impl Plugin for Md2Plugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MD2Resource::load(Path::new("assets")))
            .init_asset::<MD2>()
            .register_asset_loader(Md2Loader);
    }
}

/// Spawn a new MD2 instance
pub fn spawn_md2(
    commands: &mut Commands,
    meshes: &mut ResMut<Assets<Mesh>>,
    handle: &Handle<MD2>,
    md2s: &Res<Assets<MD2>>,
) {
    let md2 = md2s.get(handle).unwrap();
    let md2c = MD2Component {
        skin_idx: 0,
        anim_idx: 0,
        curr_frame: 0,
        interp: 0.0,
        handle: handle.clone(),
    };

    let mat3d = MeshMaterial3d(md2.get_skin(md2c.skin_idx).clone());
    let mesh_handle: Handle<Mesh> = meshes.add(md2c.create_mesh(md2));
    let scale = 1.0_f32 / 32.0_f32;
    let neg90 = f32::to_radians(-90.0);

    commands.spawn((
        Mesh3d(mesh_handle),
        mat3d,
        Transform::from_rotation(Quat::from_euler(EulerRot::ZYX, 0.0, neg90, 0.0))
            .with_scale(Vec3::splat(scale)),
        md2c,
    ));
}

/// Find all .md2 files on disk
fn find_md2(assets_path: &Path) -> Vec<PathBuf> {
    let glob_path = assets_path.join("**").join("*.md2");
    let pattern = glob_path.to_str().unwrap();
    let mut paths = Vec::new();

    for entry in glob(pattern).unwrap().filter_map(Result::ok) {
        let path = entry.strip_prefix("assets").unwrap().to_path_buf();
        paths.push(path);
    }

    paths
}

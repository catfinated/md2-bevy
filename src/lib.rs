pub mod md2 {

use std::fs::File;
use std::io::{BufReader, Read, SeekFrom, Seek};

use bevy::prelude::Vec3;


#[derive(Debug)]
#[repr(C)]
pub struct Header {
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

#[derive(Debug)]
#[repr(C)]
struct Skin {
    name: [u8; 64]
}

#[derive(Debug)]
#[repr(C)]
struct TexCoord {
    s: i16,
    t: i16,
}

#[derive(Debug)]
#[repr(C)]
struct Triangle {
    vertex: [u16; 3],
    st: [u16; 3],
}

#[derive(Debug)]
#[repr(C)]
struct Vertex {
    v: [u8; 3],
    normal_index: u8,
}

#[derive(Debug)]
#[repr(C)]
struct Frame {
    scale: [f32; 3],
    translate: [f32; 3],
    name: [u8; 16],
    // vector<Vertex>
}

#[derive(Debug)]
pub struct KeyFrame {
    pub vertices: Vec<Vec3>,
}

#[derive(Debug)]
pub struct Mesh {
    pub header: Header,
    pub key_frames: Vec<KeyFrame>,
}

impl Mesh {
    pub fn load(fpath: &String) -> Mesh {
        let inf = File::open(fpath).unwrap();
        let mut reader = BufReader::new(inf);

        // load header
        let mut buffer = [0; std::mem::size_of::<Header>()];
        reader.read_exact(&mut buffer).unwrap();
        let header: Header = unsafe { std::mem::transmute(buffer) };

        // load triangles
        let mut triangles = Vec::new();
        let num_tris = usize::try_from(header.num_tris).unwrap();
        triangles.reserve(num_tris);
        let tris_off = u64::try_from(header.offset_tris).unwrap();
        reader.seek(SeekFrom::Start(tris_off)).unwrap();

        for _ in 0..header.num_tris {
            let mut tbuf = [0; std::mem::size_of::<Triangle>()];
            reader.read_exact(&mut tbuf).unwrap();
            let triangle: Triangle = unsafe { std::mem::transmute(tbuf) };
            triangles.push(triangle);
        }

        println!("loaded {} triangles", triangles.len());

        // load frames
        let num_frames = usize::try_from(header.num_frames).unwrap();
        let mut frames: Vec<Frame> = Vec::new();
        frames.reserve(num_frames);
        let mut key_frames: Vec<KeyFrame> = Vec::new();
        key_frames.reserve(num_frames);

        let frames_off = u64::try_from(header.offset_frames).unwrap();
        reader.seek(SeekFrom::Start(frames_off)).unwrap();

        let num_xyz = usize::try_from(header.num_xyz).unwrap();

        for _ in 0..header.num_frames {
            let mut fbuf = [0; std::mem::size_of::<Frame>()];
            reader.read_exact(&mut fbuf).unwrap();
            let frame: Frame = unsafe { std::mem::transmute(fbuf) };

            let mut unscaled_vertices: Vec<Vertex> = Vec::new();
            unscaled_vertices.reserve(num_xyz);

            for _ in 0..num_xyz {
                let mut vbuf = [0; std::mem::size_of::<Vertex>()];
                reader.read_exact(&mut vbuf).unwrap();
                let vertex: Vertex = unsafe { std::mem::transmute(vbuf) };
                unscaled_vertices.push(vertex);
            }


            let mut vertices = Vec::new();
            vertices.reserve(num_tris * 3);

            for tri in &triangles {
                for i in 0..3 {
                    let vi = usize::try_from(tri.vertex[i]).unwrap();
                    let vertex = &unscaled_vertices[vi];
                    let x = (frame.scale[0] * vertex.v[0] as f32) + frame.translate[0];
                    let y = (frame.scale[1] * vertex.v[1] as f32) + frame.translate[1];
                    let z = (frame.scale[2] * vertex.v[2] as f32) + frame.translate[2];
                    vertices.push(Vec3::new(x, y, z));
                }
            }

            key_frames.push(KeyFrame{ vertices } );

        }

        Mesh{ header, key_frames }
    }
}

}

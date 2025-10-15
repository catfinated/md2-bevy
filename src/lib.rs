pub mod md2 {

    use glob::glob;
    use std::fs::File;
    use std::io::{BufReader, Read, Seek, SeekFrom};
    use std::path::{Path, PathBuf};

    use bevy::prelude::{Vec2, Vec3};

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
    }

    #[derive(Debug)]
    pub struct KeyFrame {
        pub vertices: Vec<Vec3>,
        pub normals: Vec<Vec3>,
    }

    #[derive(Debug)]
    pub struct Mesh {
        pub header: Header,
        pub key_frames: Vec<KeyFrame>,
        pub texcoords: Vec<Vec2>,
        pub skins: Vec<PathBuf>,
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

            // load texcoords
            let num_st = usize::try_from(header.num_st).unwrap();
            let st_off = u64::try_from(header.offset_st).unwrap();
            let mut unscaled_texcoords = Vec::new();
            unscaled_texcoords.reserve(num_st);
            reader.seek(SeekFrom::Start(st_off)).unwrap();

            for _ in 0..num_st {
                let mut stbuf = [0; std::mem::size_of::<TexCoord>()];
                reader.read_exact(&mut stbuf).unwrap();
                let texcoord: TexCoord = unsafe { std::mem::transmute(stbuf) };
                unscaled_texcoords.push(texcoord);
            }

            let skin_width = header.skinwidth as f32;
            let skin_height = header.skinheight as f32;

            let mut texcoords = Vec::new();
            texcoords.reserve(num_tris * 3);

            for tri in &triangles {
                for i in 0..3 {
                    let index = usize::try_from(tri.st[i]).unwrap();
                    let texcoord = &unscaled_texcoords[index];
                    let s = f32::try_from(texcoord.s).unwrap() / skin_width;
                    let t = f32::try_from(texcoord.t).unwrap() / skin_height;
                    texcoords.push(Vec2::new(s, t));
                }
            }

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
                let mut normals = Vec::new();
                normals.reserve(num_tris * 3);

                for tri in &triangles {
                    for i in 0..3 {
                        let vi = usize::try_from(tri.vertex[i]).unwrap();
                        let vertex = &unscaled_vertices[vi];
                        // NB: pay attention to the assingments here as we swap z and y
                        let x = (frame.scale[0] * vertex.v[0] as f32) + frame.translate[0];
                        let z = (frame.scale[1] * vertex.v[1] as f32) + frame.translate[1];
                        let y = (frame.scale[2] * vertex.v[2] as f32) + frame.translate[2];
                        vertices.push(Vec3::new(x, y, z));
                    }

                    let v0 = &vertices[vertices.len() - 3];
                    let v1 = &vertices[vertices.len() - 2];
                    let v2 = &vertices[vertices.len() - 1];

                    let a = v1 - v0;
                    let b = v2 - v0;
                    let c = a.cross(b);
                    let n = c.normalize();

                    normals.push(n);
                    normals.push(n);
                    normals.push(n);
                }

                key_frames.push(KeyFrame { vertices, normals });
            }

            // skins - only from directory right now
            let p = Path::new(fpath).parent().unwrap().join("*.png");
            let pat = p.as_path().to_str().unwrap();

            let mut skins = Vec::new();

            for entry in glob(pat).unwrap().filter_map(Result::ok) {
                let fpath = entry.strip_prefix("assets").unwrap();
                println!("{}", fpath.display());
                skins.push(fpath.to_path_buf());
            }

            Mesh {
                header,
                key_frames,
                texcoords,
                skins,
            }
        }
    }
}

pub mod md2 {

    use glob::glob;
    use std::fs::File;
    use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
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

    type KeyFrame = Vec<Vec3>;

    #[derive(Debug)]
    pub struct Animation {
        pub name: String,
        pub key_frames: Vec<KeyFrame>,
    }

    #[derive(Debug)]
    pub struct MD2 {
        pub animations: Vec<Animation>,
        pub texcoords: Vec<Vec2>,
        pub skins: Vec<PathBuf>,
    }

    impl Frame {
        fn get_name(&self) -> String {
            let s = String::from_utf8_lossy(&self.name);
            let t = s.trim_end_matches(|c: char| c.is_ascii_control() || c.is_ascii_digit());
            t.to_string()
        }
    }

    impl MD2 {
        pub fn load(fpath: &Path) -> MD2 {
            let inf = File::open(fpath).unwrap();
            let mut reader = BufReader::new(inf);

            // load header
            let mut buffer = [0; std::mem::size_of::<Header>()];
            reader.read_exact(&mut buffer).unwrap();
            let header: Header = unsafe { std::mem::transmute(buffer) };

            let triangles = MD2::load_triangles(&mut reader, &header);
            let texcoords = MD2::load_texcoords(&mut reader, &header, &triangles);
            let animations = MD2::load_animations(&mut reader, &header, &triangles);
            let skins = MD2::find_skins(fpath); // skins - only from directory right now

            MD2 {
                animations,
                texcoords,
                skins,
            }
        }

        fn load_triangles<R: BufRead + Seek>(reader: &mut R, header: &Header) -> Vec<Triangle> {
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

            triangles
        }

        fn load_texcoords<R: BufRead + Seek>(
            reader: &mut R,
            header: &Header,
            triangles: &Vec<Triangle>,
        ) -> Vec<Vec2> {
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
            texcoords.reserve(triangles.len() * 3);

            for tri in triangles {
                for i in 0..3 {
                    let index = usize::try_from(tri.st[i]).unwrap();
                    let texcoord = &unscaled_texcoords[index];
                    let s = f32::try_from(texcoord.s).unwrap() / skin_width;
                    let t = f32::try_from(texcoord.t).unwrap() / skin_height;
                    texcoords.push(Vec2::new(s, t));
                }
            }

            texcoords
        }

        fn read_and_decompress_vertices<R: BufRead + Seek>(
            reader: &mut R,
            num_xyz: usize,
            frame: &Frame,
            triangles: &Vec<Triangle>,
        ) -> Vec<Vec3> {
            let mut raw_vertices: Vec<Vertex> = Vec::new();
            raw_vertices.reserve(num_xyz);

            for _ in 0..num_xyz {
                let mut vbuf = [0; std::mem::size_of::<Vertex>()];
                reader.read_exact(&mut vbuf).unwrap();
                let vertex: Vertex = unsafe { std::mem::transmute(vbuf) };
                raw_vertices.push(vertex);
            }

            let mut vertices = Vec::new();
            vertices.reserve(triangles.len() * 3);

            for tri in triangles {
                for i in 0..3 {
                    let vi = usize::try_from(tri.vertex[i]).unwrap();
                    let vertex = &raw_vertices[vi];
                    // NB: pay attention to the assingments here as we swap z and y
                    let x = (frame.scale[0] * vertex.v[0] as f32) + frame.translate[0];
                    let z = (frame.scale[1] * vertex.v[1] as f32) + frame.translate[1];
                    let y = (frame.scale[2] * vertex.v[2] as f32) + frame.translate[2];
                    vertices.push(Vec3::new(x, y, z));
                }
            }

            vertices
        }

        fn load_animations<R: BufRead + Seek>(
            reader: &mut R,
            header: &Header,
            triangles: &Vec<Triangle>,
        ) -> Vec<Animation> {
            let num_xyz = usize::try_from(header.num_xyz).unwrap();
            let mut key_frames: Vec<KeyFrame> = Vec::new();
            let mut animations: Vec<Animation> = Vec::new();
            let mut last_frame_name: Option<String> = None;
            let frames_off = u64::try_from(header.offset_frames).unwrap();

            reader.seek(SeekFrom::Start(frames_off)).unwrap();

            for _ in 0..header.num_frames {
                let mut fbuf = [0; std::mem::size_of::<Frame>()];
                reader.read_exact(&mut fbuf).unwrap();
                let frame: Frame = unsafe { std::mem::transmute(fbuf) };
                let vertices =
                    MD2::read_and_decompress_vertices(reader, num_xyz, &frame, triangles);

                let curr_name = frame.get_name();
                if let Some(prev_name) = last_frame_name {
                    if prev_name != curr_name {
                        animations.push(Animation {
                            name: prev_name.clone(),
                            key_frames,
                        });

                        key_frames = Vec::new();
                    }
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

            animations
        }

        fn find_skins(fpath: &Path) -> Vec<PathBuf> {
            let glob_path = fpath.parent().unwrap().join("*.png");
            let pattern = glob_path.to_str().unwrap();
            let mut skins = Vec::new();

            for entry in glob(pattern).unwrap().filter_map(Result::ok) {
                let fpath = entry.strip_prefix("assets").unwrap();
                skins.push(fpath.to_path_buf());
            }

            skins
        }
    }
}

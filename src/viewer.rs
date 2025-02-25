use crate::urdf::*;
use k::nalgebra as na;
use kiss3d::camera::ArcBall;
use kiss3d::ncollide3d::simba::scalar::SubsetOf;
use kiss3d::scene::SceneNode;
use kiss3d::window::Window;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::rc::Rc;
use tracing::*;

pub struct Viewer {
    scenes: HashMap<String, SceneNode>,
    pub arc_ball: ArcBall,
    font: Rc<kiss3d::text::Font>,
    original_colors: HashMap<String, Vec<na::Point3<f32>>>,
    is_texture_enabled: bool,
    link_joint_map: HashMap<String, String>,
}

impl Viewer {
    pub fn new(title: &str) -> (Viewer, Window) {
        Self::with_background_color(title, (0.0, 0.0, 0.3))
    }

    pub fn with_background_color(title: &str, color: (f32, f32, f32)) -> (Viewer, Window) {
        let eye = na::Point3::new(3.0f32, 1.0, 1.0);
        let at = na::Point3::new(0.0f32, 0.0, 0.25);
        let mut window = kiss3d::window::Window::new_with_size(title, 1400, 1000);
        window.set_light(kiss3d::light::Light::StickToCamera);
        window.set_background_color(color.0, color.1, color.2);
        let mut arc_ball = ArcBall::new(eye, at);
        arc_ball.set_up_axis(na::Vector3::z());
        arc_ball.set_dist_step(0.5);
        let font = kiss3d::text::Font::default();
        let viewer = Viewer {
            scenes: HashMap::new(),
            arc_ball,
            font,
            original_colors: HashMap::new(),
            is_texture_enabled: true,
            link_joint_map: HashMap::new(),
        };
        (viewer, window)
    }
    pub fn disable_texture(&mut self) {
        self.is_texture_enabled = false;
    }
    pub fn enable_texture(&mut self) {
        self.is_texture_enabled = true;
    }

    pub fn add_robot(&mut self, window: &mut Window, urdf_robot: &urdf_rs::Robot) {
        self.add_robot_with_base_dir(window, urdf_robot, None);
    }
    pub fn add_robot_with_base_dir(
        &mut self,
        window: &mut Window,
        urdf_robot: &urdf_rs::Robot,
        base_dir: Option<&Path>,
    ) {
        self.add_robot_with_base_dir_and_collision_flag(window, urdf_robot, base_dir, false);
    }
    pub fn add_robot_with_base_dir_and_collision_flag(
        &mut self,
        window: &mut Window,
        urdf_robot: &urdf_rs::Robot,
        base_dir: Option<&Path>,
        is_collision: bool,
    ) {
        self.link_joint_map = k::urdf::link_to_joint_map(urdf_robot);

        for l in &urdf_robot.links {
            let num = if is_collision {
                l.collision.len()
            } else {
                l.visual.len()
            };
            if num == 0 {
                continue;
            }
            let mut scene_group = window.add_group();
            let mut colors = Vec::new();
            for i in 0..num {
                let (geom_element, origin_element) = if is_collision {
                    (&l.collision[i].geometry, &l.collision[i].origin)
                } else {
                    (&l.visual[i].geometry, &l.visual[i].origin)
                };
                let mut opt_color = None;
                if l.visual.len() > i {
                    let rgba = rgba_from_visual(urdf_robot, &l.visual[i]);
                    let color = na::Point3::new(rgba[0] as f32, rgba[1] as f32, rgba[2] as f32);
                    if color[0] > 0.001 || color[1] > 0.001 || color[2] > 0.001 {
                        opt_color = Some(color);
                    }
                    colors.push(color);
                }
                match add_geometry(
                    geom_element,
                    &opt_color,
                    base_dir,
                    &mut scene_group,
                    self.is_texture_enabled,
                ) {
                    Ok(mut base_group) => {
                        // set initial origin offset
                        base_group.set_local_transformation(k::urdf::isometry_from(origin_element));
                    }
                    Err(e) => {
                        error!("failed to create for link '{}': {e}", l.name);
                    }
                }
            }
            let joint_name = self
                .link_joint_map
                .get(&l.name)
                .unwrap_or_else(|| panic!("joint for link '{}' not found", l.name));
            self.scenes.insert(joint_name.to_owned(), scene_group);
            self.original_colors.insert(joint_name.to_owned(), colors);
        }
    }
    pub fn remove_robot(&mut self, window: &mut Window, urdf_robot: &urdf_rs::Robot) {
        for l in &urdf_robot.links {
            let joint_name = self
                .link_joint_map
                .get(&l.name)
                .unwrap_or_else(|| panic!("{} not found", l.name));
            if let Some(scene) = self.scenes.get_mut(joint_name) {
                window.remove_node(scene);
            }
        }
    }
    pub fn add_axis_cylinders(&mut self, window: &mut Window, name: &str, size: f32) {
        let mut axis_group = window.add_group();
        let mut x = axis_group.add_cylinder(0.01, size);
        x.set_color(0.0, 0.0, 1.0);
        let mut y = axis_group.add_cylinder(0.01, size);
        y.set_color(0.0, 1.0, 0.0);
        let mut z = axis_group.add_cylinder(0.01, size);
        z.set_color(1.0, 0.0, 0.0);
        let rot_x = na::UnitQuaternion::from_axis_angle(&na::Vector3::x_axis(), 1.57);
        let rot_y = na::UnitQuaternion::from_axis_angle(&na::Vector3::y_axis(), 1.57);
        let rot_z = na::UnitQuaternion::from_axis_angle(&na::Vector3::z_axis(), 1.57);
        x.append_translation(&na::Translation3::new(0.0, 0.0, size * 0.5));
        y.append_translation(&na::Translation3::new(0.0, size * 0.5, 0.0));
        z.append_translation(&na::Translation3::new(size * 0.5, 0.0, 0.0));
        x.set_local_rotation(rot_x);
        y.set_local_rotation(rot_y);
        z.set_local_rotation(rot_z);
        self.scenes.insert(name.to_owned(), axis_group);
    }
    pub fn scene_node(&self, name: &str) -> Option<&SceneNode> {
        self.scenes.get(name)
    }
    pub fn scene_node_mut(&mut self, name: &str) -> Option<&mut SceneNode> {
        self.scenes.get_mut(name)
    }
    pub fn update<T>(&mut self, robot: &k::Chain<T>)
    where
        T: k::RealField + k::SubsetOf<f64> + kiss3d::ncollide3d::simba::scalar::SubsetOf<f32>,
    {
        robot.update_transforms();
        for link in robot.iter() {
            let trans = link.world_transform().unwrap();
            let link_name = &link.joint().name;
            let trans_f32: na::Isometry3<f32> = na::Isometry3::to_superset(&trans);
            match self.scenes.get_mut(link_name) {
                Some(obj) => {
                    obj.set_local_transformation(trans_f32);
                }
                None => {
                    debug!("{link_name} not found");
                }
            }
        }
    }
    pub fn draw_text(
        &mut self,
        window: &mut Window,
        text: &str,
        size: f32,
        pos: &na::Point2<f32>,
        color: &na::Point3<f32>,
    ) {
        window.draw_text(text, pos, size, &self.font, color);
    }
    pub fn set_temporal_color(&mut self, link_name: &str, r: f32, g: f32, b: f32) {
        if let Some(obj) = self.scenes.get_mut(link_name) {
            obj.set_color(r, g, b);
        }
    }
    pub fn reset_temporal_color(&mut self, link_name: &str) {
        if let Some(colors) = self.original_colors.get(link_name) {
            if let Some(obj) = self.scenes.get_mut(link_name) {
                for color in colors {
                    obj.apply_to_scene_nodes_mut(&mut |o| {
                        o.set_color(color[0], color[1], color[2]);
                    });
                }
            }
        }
    }
    pub fn add_ground(
        &mut self,
        window: &mut Window,
        ground_height: f32,
        panel_size: f32,
        half_panel_num: i32,
        ground_color1: (f32, f32, f32),
        ground_color2: (f32, f32, f32),
    ) -> Vec<SceneNode> {
        let mut panels = Vec::new();
        const PANEL_HEIGHT: f32 = 0.0001;

        for i in -half_panel_num..half_panel_num {
            for j in -half_panel_num..half_panel_num {
                let mut c0 = window.add_cube(panel_size, panel_size, PANEL_HEIGHT);
                if (i + j) % 2 == 0 {
                    c0.set_color(ground_color1.0, ground_color1.1, ground_color1.2);
                } else {
                    c0.set_color(ground_color2.0, ground_color2.1, ground_color2.2);
                }
                let x_ind = j as f32 + 0.5;
                let y_ind = i as f32 + 0.5;
                let trans = na::Isometry3::from_parts(
                    na::Translation3::new(
                        panel_size * x_ind,
                        panel_size * y_ind,
                        ground_height - PANEL_HEIGHT * 0.5,
                    ),
                    na::UnitQuaternion::identity(),
                );
                c0.set_local_transformation(trans);
                panels.push(c0);
            }
        }
        panels
    }

    //https://docs.rs/kiss3d/latest/kiss3d/scene/struct.SceneNode.html#method.add_cube
    pub(crate) fn add_cube(&mut self, window: &mut Window, name: &str, wx: f32, wy: f32, wz: f32) {
        let cube = window.add_cube(wx, wy, wz);
        self.scenes.insert(name.to_owned(), cube);
    }

    pub(crate) fn add_capsule(
        &mut self,
        window: &mut Window,
        name: &str,
        radius: f32,
        height: f32,
    ) {
        let cube = window.add_capsule(radius, height);
        self.scenes.insert(name.to_owned(), cube);
    }
}

impl fmt::Debug for Viewer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // kiss3d::scene::SceneNode and kiss3d::text::Font don't implement Debug.
        f.debug_struct("Viewer")
            .field("scenes", &self.scenes.keys())
            .field("original_colors", &self.original_colors)
            .field("is_texture_enabled", &self.is_texture_enabled)
            .field("link_joint_map", &self.link_joint_map)
            .finish()
    }
}

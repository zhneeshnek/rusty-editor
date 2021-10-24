use crate::rg3d::core::math::Matrix4Ext;
use rg3d::core::algebra::Matrix4;
use rg3d::core::math::plane::Plane;
use rg3d::{
    core::{
        algebra::{Point3, UnitQuaternion, Vector2, Vector3},
        math::aabb::AxisAlignedBoundingBox,
        pool::Handle,
    },
    gui::message::{KeyCode, MouseButton},
    scene::{
        base::BaseBuilder, camera::CameraBuilder, graph::Graph, node::Node,
        transform::TransformBuilder,
    },
};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
};
use std::cell::Cell;

pub struct CameraController {
    pub pivot: Handle<Node>,
    pub camera: Handle<Node>,
    yaw: f32,
    pitch: f32,
    rotate: Cell<bool>,
    drag_side: f32,
    drag_up: f32,
    drag: Cell<bool>,
    move_left: Cell<bool>,
    move_right: Cell<bool>,
    move_forward: Cell<bool>,
    move_backward: Cell<bool>,
    move_up: Cell<bool>,
    move_down: Cell<bool>,
    speed_factor: Cell<f32>,
    stack: Vec<Handle<Node>>,
    editor_context: PickContext,
    scene_context: PickContext,
}

#[derive(Clone)]
pub struct CameraPickResult {
    pub position: Vector3<f32>,
    pub node: Handle<Node>,
    pub toi: f32,
}

#[derive(Default)]
struct PickContext {
    pick_list: Vec<CameraPickResult>,
    pick_index: usize,
    old_selection_hash: u64,
    old_cursor_pos: Vector2<f32>,
}

impl CameraController {
    pub fn new(graph: &mut Graph, root: Handle<Node>) -> Self {
        let camera;
        let pivot = BaseBuilder::new()
            .with_children(&[{
                camera = CameraBuilder::new(BaseBuilder::new().with_name("EditorCamera"))
                    .with_z_far(512.0)
                    .build(graph);
                camera
            }])
            .with_name("EditorCameraPivot")
            .with_local_transform(
                TransformBuilder::new()
                    .with_local_position(Vector3::new(0.0, 1.0, -3.0))
                    .build(),
            )
            .build(graph);

        graph.link_nodes(pivot, root);

        Self {
            pivot,
            camera,
            yaw: 0.0,
            pitch: 0.0,
            rotate: Cell::new(false),
            drag_side: 0.0,
            drag_up: 0.0,
            drag: Cell::new(false),
            move_left: Cell::new(false),
            move_right: Cell::new(false),
            move_forward: Cell::new(false),
            move_backward: Cell::new(false),
            move_up: Cell::new(false),
            move_down: Cell::new(false),
            speed_factor: Cell::new(1.0),
            stack: Default::default(),
            editor_context: Default::default(),
            scene_context: Default::default(),
        }
    }

    pub fn on_mouse_move(&mut self, delta: Vector2<f32>) {
        if self.rotate.get() {
            self.yaw -= delta.x as f32 * 0.01;
            self.pitch += delta.y as f32 * 0.01;
            if self.pitch > 90.0f32.to_radians() {
                self.pitch = 90.0f32.to_radians();
            }
            if self.pitch < -90.0f32.to_radians() {
                self.pitch = -90.0f32.to_radians();
            }
        }

        if self.drag.get() {
            self.drag_side -= delta.x * 0.01;
            self.drag_up -= delta.y * 0.01;
        }
    }

    pub fn on_mouse_wheel(&self, delta: f32, graph: &mut Graph) {
        let camera = &mut graph[self.camera];

        let look = camera.global_transform().look();

        if let Node::Base(pivot) = &mut graph[self.pivot] {
            pivot.local_transform_mut().offset(look.scale(delta));
        }
    }

    pub fn on_mouse_button_up(&self, button: MouseButton) {
        match button {
            MouseButton::Right => {
                self.rotate.set(false);
            }
            MouseButton::Middle => {
                self.drag.set(false);
            }
            _ => (),
        }
    }

    pub fn on_mouse_button_down(&self, button: MouseButton) {
        match button {
            MouseButton::Right => {
                self.rotate.set(true);
            }
            MouseButton::Middle => {
                self.drag.set(true);
            }
            _ => (),
        }
    }

    pub fn on_key_up(&self, key: KeyCode) {
        match key {
            KeyCode::W => self.move_forward.set(false),
            KeyCode::S => self.move_backward.set(false),
            KeyCode::A => self.move_left.set(false),
            KeyCode::D => self.move_right.set(false),
            KeyCode::Space | KeyCode::Q => self.move_up.set(false),
            KeyCode::E => self.move_down.set(false),
            KeyCode::LControl | KeyCode::LShift => self.speed_factor.set(1.0),
            _ => (),
        }
    }

    pub fn on_key_down(&self, key: KeyCode) {
        match key {
            KeyCode::W => self.move_forward.set(true),
            KeyCode::S => self.move_backward.set(true),
            KeyCode::A => self.move_left.set(true),
            KeyCode::D => self.move_right.set(true),
            KeyCode::Space | KeyCode::Q => self.move_up.set(true),
            KeyCode::E => self.move_down.set(true),
            KeyCode::LControl => self.speed_factor.set(2.0),
            KeyCode::LShift => self.speed_factor.set(0.25),
            _ => (),
        }
    }

    pub fn update(&mut self, graph: &mut Graph, dt: f32) {
        let camera = &mut graph[self.camera];

        let global_transform = camera.global_transform();
        let look = global_transform.look();
        let side = global_transform.side();
        let up = global_transform.up();

        let mut move_vec = Vector3::default();
        if self.move_forward.get() {
            move_vec += look;
        }
        if self.move_backward.get() {
            move_vec -= look;
        }
        if self.move_left.get() {
            move_vec += side;
        }
        if self.move_right.get() {
            move_vec -= side;
        }
        if self.move_up.get() {
            move_vec += up;
        }
        if self.move_down.get() {
            move_vec -= up;
        }
        if let Some(v) = move_vec.try_normalize(std::f32::EPSILON) {
            move_vec = v.scale(self.speed_factor.get() * 10.0 * dt);
        }

        move_vec += side * self.drag_side;
        move_vec.y += self.drag_up;
        self.drag_side = 0.0;
        self.drag_up = 0.0;

        if let Node::Camera(camera) = camera {
            let pitch = UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.pitch);
            camera.local_transform_mut().set_rotation(pitch);
        }
        if let Node::Base(pivot) = &mut graph[self.pivot] {
            let yaw = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.yaw);
            pivot
                .local_transform_mut()
                .set_rotation(yaw)
                .offset(move_vec);
        }
    }

    pub fn pick<F>(
        &mut self,
        cursor_pos: Vector2<f32>,
        graph: &Graph,
        root: Handle<Node>,
        screen_size: Vector2<f32>,
        editor_only: bool,
        mut filter: F,
    ) -> Option<CameraPickResult>
    where
        F: FnMut(Handle<Node>, &Node) -> bool,
    {
        let camera = &graph[self.camera];
        if let Node::Camera(camera) = camera {
            let ray = camera.make_ray(cursor_pos, screen_size);

            self.stack.clear();
            let context = if editor_only {
                // In case if we want to pick stuff from editor scene only, we have to
                // start traversing graph from editor root.
                self.stack.push(root);
                &mut self.editor_context
            } else {
                self.stack.push(graph.get_root());
                &mut self.scene_context
            };

            context.pick_list.clear();

            while let Some(handle) = self.stack.pop() {
                // Ignore editor nodes if we picking scene stuff only.
                if !editor_only && handle == root {
                    continue;
                }

                let node = &graph[handle];

                self.stack.extend_from_slice(node.children());

                if !node.global_visibility() || !filter(handle, node) {
                    continue;
                }

                let (aabb, surfaces) = match node {
                    Node::Mesh(mesh) => (mesh.bounding_box(), Some(mesh.surfaces())),
                    Node::Base(_) if handle == graph.get_root() || handle == root => {
                        (AxisAlignedBoundingBox::default(), None)
                    }
                    _ => (AxisAlignedBoundingBox::unit(), None),
                };

                if handle != graph.get_root() {
                    let object_space_ray =
                        ray.transform(node.global_transform().try_inverse().unwrap_or_default());
                    // Do coarse intersection test with bounding box.
                    if let Some(points) = object_space_ray.aabb_intersection_points(&aabb) {
                        // Do fine intersection test with surfaces if any
                        if let Some(_surfaces) = surfaces {
                            // TODO
                        }

                        let da = points[0].metric_distance(&object_space_ray.origin);
                        let db = points[1].metric_distance(&object_space_ray.origin);
                        let closest_distance = da.min(db);
                        context.pick_list.push(CameraPickResult {
                            position: node
                                .global_transform()
                                .transform_point(&Point3::from(if da < db {
                                    points[0]
                                } else {
                                    points[1]
                                }))
                                .coords,
                            node: handle,
                            toi: closest_distance,
                        });
                    }
                }
            }

            // Make sure closest will be selected first.
            context
                .pick_list
                .sort_by(|a, b| a.toi.partial_cmp(&b.toi).unwrap());

            let mut hasher = DefaultHasher::new();
            for result in context.pick_list.iter() {
                result.node.hash(&mut hasher);
            }
            let selection_hash = hasher.finish();
            if selection_hash == context.old_selection_hash && cursor_pos == context.old_cursor_pos
            {
                context.pick_index += 1;

                // Wrap picking loop.
                if context.pick_index >= context.pick_list.len() {
                    context.pick_index = 0;
                }
            } else {
                // Select is different, start from beginning.
                context.pick_index = 0;
            }
            context.old_selection_hash = selection_hash;
            context.old_cursor_pos = cursor_pos;

            if !context.pick_list.is_empty() {
                if let Some(result) = context.pick_list.get(context.pick_index) {
                    return Some(result.clone());
                }
            }
        }

        None
    }

    pub fn pick_on_plane(
        &self,
        plane: Plane,
        graph: &Graph,
        mouse_position: Vector2<f32>,
        viewport_size: Vector2<f32>,
        transform: Matrix4<f32>,
    ) -> Option<Vector3<f32>> {
        if let Node::Camera(camera) = &graph[self.camera] {
            camera
                .make_ray(mouse_position, viewport_size)
                .transform(transform)
                .plane_intersection_point(&plane)
        } else {
            unreachable!()
        }
    }
}

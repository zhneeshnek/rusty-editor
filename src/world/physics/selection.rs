use crate::physics::{Joint, RigidBody};
use crate::{physics::Physics, utils};
use rg3d::core::{algebra::Vector3, pool::Handle};

#[derive(Debug, Clone)]
pub struct RigidBodySelection {
    pub bodies: Vec<Handle<RigidBody>>,
}

impl RigidBodySelection {
    pub fn bodies(&self) -> &[Handle<RigidBody>] {
        &self.bodies
    }

    pub fn is_single_selection(&self) -> bool {
        self.bodies.len() == 1
    }

    pub fn first(&self) -> Option<Handle<RigidBody>> {
        self.bodies.first().cloned()
    }

    pub fn center(&self, physics: &Physics) -> Option<Vector3<f32>> {
        let mut count = 0;
        let position_sum = self.bodies.iter().fold(Vector3::default(), |acc, handle| {
            count += 1;
            acc + physics.bodies[*handle].position
        });
        if count > 0 {
            Some(position_sum.scale(1.0 / count as f32))
        } else {
            None
        }
    }
}

impl PartialEq for RigidBodySelection {
    fn eq(&self, other: &Self) -> bool {
        utils::is_slice_equal_permutation(self.bodies(), other.bodies())
    }
}

impl Eq for RigidBodySelection {}

#[derive(Debug, Clone)]
pub struct JointSelection {
    pub joints: Vec<Handle<Joint>>,
}

impl JointSelection {
    pub fn joints(&self) -> &[Handle<Joint>] {
        &self.joints
    }

    pub fn is_single_selection(&self) -> bool {
        self.joints.len() == 1
    }

    pub fn first(&self) -> Option<Handle<Joint>> {
        self.joints.first().cloned()
    }
}

impl PartialEq for JointSelection {
    fn eq(&self, other: &Self) -> bool {
        utils::is_slice_equal_permutation(self.joints(), other.joints())
    }
}

impl Eq for JointSelection {}
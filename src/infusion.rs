use std::collections::HashMap;
use std::rc::Rc;

#[derive(Debug)]
pub enum InfusionType {
    Drug,
    Solution
}

#[derive(Debug, PartialEq, Eq)]
pub enum Compatibility {
    Compatible,
    Incompatible,
}

#[derive(Debug)]
pub struct CompatibilityData {
    compatible: u32,
    incompatible: u32,
    mixed: u32,
    compatibility: Compatibility,
}

impl CompatibilityData {
    pub fn new(compatible: u32, incompatible: u32, mixed: u32) -> CompatibilityData {
        let compatibility = if compatible > 0 && incompatible == 0 && mixed == 0 {
            Compatibility::Compatible
        } else {
            Compatibility::Incompatible
        };

        Self {
            compatible,
            incompatible,
            mixed,
            compatibility,
        }
    }
}

#[derive(Debug)]
pub struct Infusion {
    id: u32,
    name: String,
    infusion_type: InfusionType,
    compatibility: HashMap<u32, Rc<CompatibilityData>>, // Infusion.id -> CompatibilityData
}

impl Infusion {
    pub fn new(id: u32, name: String, infusion_type: InfusionType) -> Self {
        Self {
            id,
            name,
            infusion_type,
            compatibility: HashMap::new(),
        }
    }

    pub fn id(&self) -> u32 {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn add_compatibility_data(&mut self, other_id: u32, compat_data: &Rc<CompatibilityData>) {
        self.compatibility.insert(other_id, Rc::clone(compat_data));
    }

    pub fn get_compatible(&self) -> impl Iterator<Item = &u32> {
        self.compatibility.keys().filter(
            |id| {
                let compat = self.compatibility.get(id).unwrap();
                compat.compatibility == Compatibility::Compatible
            }
        )
    }

    pub fn get_incompatible(&self, all_ids: &Vec<&u32>) -> Vec<u32> {
        let compat: Vec<_> = self.get_compatible().map(|i| { i.clone() }).collect();
        let mut incompat = Vec::new();

        for id in all_ids {
            let id = *id;
            if !compat.contains(id) {
                incompat.push(id.clone());
            }
        }
        
        incompat
    }
}
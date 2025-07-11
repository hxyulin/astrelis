use std::any::Any;

use super::{Entity, Registry};

pub trait Component: Any {}

pub trait ComponentRef {
    fn add_components(self, ent: Entity, reg: &mut Registry);
}

impl<T> ComponentRef for T
where
    T: Component,
{
    fn add_components(self, ent: Entity, reg: &mut Registry) {
        reg.add_component(ent, self);
    }
}

impl<T0> ComponentRef for (T0,)
where
    T0: Component,
{
    fn add_components(self, ent: Entity, reg: &mut Registry) {
        reg.add_component(ent, self.0);
    }
}

impl<T0, T1> ComponentRef for (T0, T1)
where
    T0: Component,
    T1: Component,
{
    fn add_components(self, ent: Entity, reg: &mut Registry) {
        reg.add_component(ent, self.0);
        reg.add_component(ent, self.1);
    }
}

impl<T0, T1, T2> ComponentRef for (T0, T1, T2)
where
    T0: Component,
    T1: Component,
    T2: Component,
{
    fn add_components(self, ent: Entity, reg: &mut Registry) {
        reg.add_component(ent, self.0);
        reg.add_component(ent, self.1);
        reg.add_component(ent, self.2);
    }
}

impl<T0, T1, T2, T3> ComponentRef for (T0, T1, T2, T3)
where
    T0: Component,
    T1: Component,
    T2: Component,
    T3: Component,
{
    fn add_components(self, ent: Entity, reg: &mut Registry) {
        reg.add_component(ent, self.0);
        reg.add_component(ent, self.1);
        reg.add_component(ent, self.2);
        reg.add_component(ent, self.3);
    }
}

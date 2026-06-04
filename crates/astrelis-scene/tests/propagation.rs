//! Dirty-pass propagation must produce results identical to a
//! brute-force recompute, under randomized mutation sequences.

use astrelis_core::math::{Mat4, Quat, Vec3};
use astrelis_scene::{NodeId, Scene, Transform};

/// Deterministic LCG so failures reproduce; no rand dependency.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        self.0 >> 33
    }

    fn pick(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }

    fn f32(&mut self) -> f32 {
        (self.next() % 2000) as f32 / 100.0 - 10.0
    }
}

/// Ground truth: walk up the parent chain multiplying local matrices.
fn brute_world(scene: &Scene, id: NodeId) -> Mat4 {
    let local = scene.local_transform(id).unwrap().matrix();
    match scene.parent(id) {
        Some(p) => brute_world(scene, p) * local,
        None => local,
    }
}

/// Ground truth visibility: AND of own flag and all ancestors'.
fn brute_visible(scene: &Scene, id: NodeId) -> bool {
    let own = scene.visible(id).unwrap();
    match scene.parent(id) {
        Some(p) => own && brute_visible(scene, p),
        None => own,
    }
}

fn live_nodes(scene: &Scene) -> Vec<NodeId> {
    let mut out = Vec::new();
    for &root in scene.roots() {
        out.push(root);
        out.extend(scene.descendants(root));
    }
    out
}

#[test]
fn dirty_pass_matches_brute_force_under_random_mutations() {
    for seed in 0..10u64 {
        let mut rng = Lcg(seed.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(1));
        let mut scene = Scene::new();
        // Seed with a few roots.
        for _ in 0..3 {
            let _ = scene.spawn();
        }

        for step in 0..200 {
            let nodes = live_nodes(&scene);
            match rng.pick(6) {
                // Spawn (child of a random node, or a new root).
                0 => {
                    if !nodes.is_empty() && rng.pick(4) != 0 {
                        let parent = nodes[rng.pick(nodes.len())];
                        let _ = scene.spawn_child(parent);
                    } else {
                        let _ = scene.spawn();
                    }
                }
                // Despawn a random subtree (keep at least one node).
                1 => {
                    if nodes.len() > 1 {
                        scene.despawn(nodes[rng.pick(nodes.len())]);
                    }
                }
                // Random local transform. (set_position/set_rotation/
                // set_scale share set_transform's dirty-marking path, so
                // the harness intentionally fuzzes only set_transform.)
                2 => {
                    if !nodes.is_empty() {
                        let id = nodes[rng.pick(nodes.len())];
                        scene.set_transform(
                            id,
                            Transform {
                                position: Vec3::new(rng.f32(), rng.f32(), rng.f32()),
                                rotation: Quat::from_rotation_z(rng.f32()),
                                // Clamp scale: deep chains of scale ~10 would
                                // push matrix elements past f32 precision at
                                // the 1e-3 absolute epsilon and flake.
                                scale: Vec3::splat(rng.f32().abs().clamp(0.1, 2.0)),
                            },
                        );
                    }
                }
                // Toggle visibility.
                3 => {
                    if !nodes.is_empty() {
                        let id = nodes[rng.pick(nodes.len())];
                        let v = scene.visible(id).unwrap();
                        scene.set_visible(id, !v);
                    }
                }
                // Reparent (cycles rejected by the API — ignore errors).
                4 => {
                    if nodes.len() >= 2 {
                        let id = nodes[rng.pick(nodes.len())];
                        let target = if rng.pick(5) == 0 {
                            None
                        } else {
                            Some(nodes[rng.pick(nodes.len())])
                        };
                        let _ = scene.set_parent(id, target);
                    }
                }
                // Flush mid-sequence so the dirty state is exercised
                // across multiple passes, not just one big one.
                _ => scene.flush_transforms(),
            }

            // Every 20 steps: flush and compare everything to ground truth.
            if step % 20 == 19 {
                scene.flush_transforms();
                for id in live_nodes(&scene) {
                    let cached = scene.world_transform(id).unwrap();
                    let truth = brute_world(&scene, id);
                    assert!(
                        cached.abs_diff_eq(truth, 1e-3),
                        "seed {seed} step {step}: world mismatch for {id:?}\ncached: {cached}\ntruth: {truth}"
                    );
                    assert_eq!(
                        scene.is_world_visible(id).unwrap(),
                        brute_visible(&scene, id),
                        "seed {seed} step {step}: visibility mismatch for {id:?}"
                    );
                }
            }
        }
    }
}

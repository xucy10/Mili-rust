use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap, HashSet};

use valence_protocol::BlockPos;
use valence_server::layer::chunk::ChunkLayer;

/// A node in the A* pathfinding search.
#[derive(Clone, Eq, PartialEq)]
struct PathNode {
    pos: BlockPos,
    g_cost: u32,
    h_cost: u32,
    parent: Option<BlockPos>,
}

impl PathNode {
    fn f_cost(&self) -> u32 {
        self.g_cost + self.h_cost
    }
}

impl Ord for PathNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f_cost()
            .cmp(&self.f_cost())
            .then_with(|| self.h_cost.cmp(&other.h_cost))
    }
}

impl PartialOrd for PathNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Context for pathfinding.
pub struct PathfindingContext<'a> {
    pub chunk_layer: &'a ChunkLayer,
    pub start: BlockPos,
    pub end: BlockPos,
    pub max_iterations: u32,
    pub allow_swim: bool,
    pub allow_climb: bool,
    pub entity_height: f32,
    pub entity_width: f32,
}

/// Result of a pathfinding operation.
#[derive(Debug, Clone)]
pub struct PathfindingResult {
    /// The path from start to end (including start and end).
    pub path: Vec<BlockPos>,
    /// Whether the path reached the target exactly.
    pub reached_target: bool,
    /// Number of iterations performed.
    pub iterations: u32,
}

/// Find a path from start to end using A*.
pub fn find_path(ctx: &PathfindingContext) -> Option<Vec<BlockPos>> {
    find_path_detailed(ctx).map(|r| r.path)
}

/// Find a path with detailed result information.
pub fn find_path_detailed(ctx: &PathfindingContext) -> Option<PathfindingResult> {
    let mut open_set: BinaryHeap<PathNode> = BinaryHeap::new();
    let mut closed_set: HashSet<BlockPos> = HashSet::new();
    let mut g_costs: HashMap<BlockPos, u32> = HashMap::new();
    let mut parent_map: HashMap<BlockPos, BlockPos> = HashMap::new();

    let start_node = PathNode {
        pos: ctx.start,
        g_cost: 0,
        h_cost: heuristic(ctx.start, ctx.end),
        parent: None,
    };

    g_costs.insert(ctx.start, 0);
    open_set.push(start_node);

    let mut iterations = 0;
    let mut best_node: Option<PathNode> = None;
    let mut best_distance = u32::MAX;

    while let Some(current) = open_set.pop() {
        // Check if we reached the target
        if current.pos == ctx.end {
            let path = reconstruct_path(&parent_map, current.pos);
            return Some(PathfindingResult {
                path,
                reached_target: true,
                iterations,
            });
        }

        // Track closest node to target (for partial paths)
        let dist_to_target = heuristic(current.pos, ctx.end);
        if dist_to_target < best_distance {
            best_distance = dist_to_target;
            best_node = Some(current.clone());
        }

        if iterations >= ctx.max_iterations {
            // Return best partial path found
            if let Some(ref best) = best_node {
                let path = reconstruct_path(&parent_map, best.pos);
                return Some(PathfindingResult {
                    path,
                    reached_target: false,
                    iterations,
                });
            }
            return None;
        }
        iterations += 1;

        if !closed_set.insert(current.pos) {
            continue;
        }

        // Check all walkable neighbors
        for (neighbor_pos, cost) in get_walkable_neighbors(ctx, current.pos) {
            if closed_set.contains(&neighbor_pos) {
                continue;
            }

            let tentative_g = current.g_cost + cost;

            if let Some(&best_g) = g_costs.get(&neighbor_pos) {
                if tentative_g >= best_g {
                    continue;
                }
            }

            g_costs.insert(neighbor_pos, tentative_g);
            parent_map.insert(neighbor_pos, current.pos);

            let neighbor_node = PathNode {
                pos: neighbor_pos,
                g_cost: tentative_g,
                h_cost: heuristic(neighbor_pos, ctx.end),
                parent: Some(current.pos),
            };

            open_set.push(neighbor_node);
        }
    }

    // Return best partial path if we found one
    if let Some(ref best) = best_node {
        let path = reconstruct_path(&parent_map, best.pos);
        return Some(PathfindingResult {
            path,
            reached_target: false,
            iterations,
        });
    }

    None
}

/// Heuristic function for A* (Manhattan distance with diagonal bonus).
fn heuristic(a: BlockPos, b: BlockPos) -> u32 {
    let dx = (a.x - b.x).unsigned_abs();
    let dy = (a.y - b.y).unsigned_abs();
    let dz = (a.z - b.z).unsigned_abs();

    // Manhattan distance * 10 (base cost)
    (dx + dy + dz) as u32 * 10
}

/// Move cost between two adjacent positions.
fn move_cost(a: BlockPos, b: BlockPos) -> u32 {
    let dx = (a.x - b.x).unsigned_abs();
    let dy = (a.y - b.y).unsigned_abs();
    let dz = (a.z - b.z).unsigned_abs();

    let manhattan = dx + dy + dz;

    if manhattan == 0 {
        0
    } else if manhattan == 1 {
        // Cardinal direction
        if dy == 1 {
            11 // Climbing cost (slightly more expensive)
        } else {
            10 // Walking cost
        }
    } else if manhattan == 2 {
        // Diagonal
        14 // ~10 * sqrt(2)
    } else {
        10 * manhattan as u32
    }
}

/// Reconstruct the path from the parent map.
fn reconstruct_path(parent_map: &HashMap<BlockPos, BlockPos>, end: BlockPos) -> Vec<BlockPos> {
    let mut path = vec![end];
    let mut current = end;

    while let Some(&parent) = parent_map.get(&current) {
        path.push(parent);
        current = parent;
    }

    path.reverse();
    path
}

/// Get walkable neighbors for a given position.
/// Returns (position, cost) pairs.
fn get_walkable_neighbors(ctx: &PathfindingContext, pos: BlockPos) -> Vec<(BlockPos, u32)> {
    let mut neighbors = Vec::with_capacity(12);

    // Check 4 cardinal directions on same Y level
    let horizontal_dirs = [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1)];

    for &(dx, dy, dz) in &horizontal_dirs {
        let neighbor = BlockPos::new(pos.x + dx, pos.y + dy, pos.z + dz);
        if is_walkable(ctx, neighbor) {
            neighbors.push((neighbor, 10));
        }
    }

    // Check for climbing (one block up)
    for &(dx, _, dz) in &horizontal_dirs {
        let above = BlockPos::new(pos.x + dx, pos.y + 1, pos.z + dz);
        let on_top = BlockPos::new(pos.x + dx, pos.y + 2, pos.z + dz);

        if ctx.allow_climb && is_walkable(ctx, above) && is_passable(ctx, on_top) {
            neighbors.push((above, 11)); // Climbing is slightly more expensive
        }
    }

    // Check for falling (one block down) on each horizontal direction
    for &(dx, _, dz) in &horizontal_dirs {
        let below = BlockPos::new(pos.x + dx, pos.y - 1, pos.z + dz);
        if is_solid(ctx, below) && is_passable(ctx, BlockPos::new(pos.x + dx, pos.y, pos.z + dz)) {
            neighbors.push((BlockPos::new(pos.x + dx, pos.y, pos.z + dz), 10));
        }
    }

    // Check falling straight down
    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
    if is_passable(ctx, below) {
        // Find the ground below
        let mut ground_y = pos.y - 1;
        while ground_y > ctx.chunk_layer.min_y() {
            let check = BlockPos::new(pos.x, ground_y, pos.z);
            if is_solid(ctx, check) {
                break;
            }
            ground_y -= 1;
        }
        let land_pos = BlockPos::new(pos.x, ground_y + 1, pos.z);
        if land_pos != pos && is_passable(ctx, land_pos) {
            neighbors.push((land_pos, 10));
        }
    }

    neighbors
}

/// Check if a position is walkable (has solid block below and passable space above).
fn is_walkable(ctx: &PathfindingContext, pos: BlockPos) -> bool {
    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);

    // Must have a solid block below (or be at the world floor)
    if !is_solid(ctx, below) && pos.y > ctx.chunk_layer.min_y() {
        return false;
    }

    // Must be passable at the entity's feet
    if !is_passable(ctx, pos) {
        return false;
    }

    // Must be passable at the entity's head (for tall entities)
    let head_pos = BlockPos::new(pos.x, pos.y + 1, pos.z);
    if !is_passable(ctx, head_pos) {
        return false;
    }

    true
}

/// Check if a position is passable (air, water, etc. - not solid).
fn is_passable(ctx: &PathfindingContext, pos: BlockPos) -> bool {
    match ctx.chunk_layer.block(pos) {
        Some(block_ref) => {
            let state = block_ref.state;
            // Passable blocks: air, water (if swimming allowed), plants, etc.
            state.is_air() || state.is_replaceable() || (ctx.allow_swim && state.is_liquid())
        }
        None => false, // Beyond world border = not passable
    }
}

/// Check if a position contains a solid block.
fn is_solid(ctx: &PathfindingContext, pos: BlockPos) -> bool {
    match ctx.chunk_layer.block(pos) {
        Some(block_ref) => {
            let state = block_ref.state;
            state.blocks_motion() || state.is_opaque()
        }
        None => false,
    }
}

/// Reconstruct the path from a PathNode chain (legacy helper).
fn reconstruct_path_legacy(node: &PathNode) -> Vec<BlockPos> {
    let mut path = vec![node.pos];
    let mut current = node;

    while let Some(parent_pos) = current.parent {
        path.push(parent_pos);
        break; // In a real implementation, we'd look up the parent node from the map.
    }

    path.reverse();
    path
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_heuristic() {
        let a = BlockPos::new(0, 0, 0);
        let b = BlockPos::new(3, 4, 0);
        assert_eq!(heuristic(a, b), 70); // (3+4+0) * 10
    }

    #[test]
    fn test_move_cost_cardinal() {
        let a = BlockPos::new(0, 0, 0);
        let b = BlockPos::new(1, 0, 0);
        assert_eq!(move_cost(a, b), 10);
    }

    #[test]
    fn test_move_cost_diagonal() {
        let a = BlockPos::new(0, 0, 0);
        let b = BlockPos::new(1, 0, 1);
        assert_eq!(move_cost(a, b), 14);
    }

    #[test]
    fn test_move_cost_climb() {
        let a = BlockPos::new(0, 0, 0);
        let b = BlockPos::new(0, 1, 0);
        assert_eq!(move_cost(a, b), 11);
    }
}

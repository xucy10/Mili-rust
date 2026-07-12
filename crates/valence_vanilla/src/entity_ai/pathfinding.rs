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
            if let Some(best) = &best_node {
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
    if let Some(best) = &best_node {
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

/// Get all walkable neighbors of a position.
fn get_walkable_neighbors(ctx: &PathfindingContext, pos: BlockPos) -> Vec<(BlockPos, u32)> {
    let mut neighbors = Vec::new();
    let directions = [
        BlockPos::new(pos.x + 1, pos.y, pos.z),
        BlockPos::new(pos.x - 1, pos.y, pos.z),
        BlockPos::new(pos.x, pos.y, pos.z + 1),
        BlockPos::new(pos.x, pos.y, pos.z - 1),
        BlockPos::new(pos.x, pos.y + 1, pos.z),
        BlockPos::new(pos.x, pos.y - 1, pos.z),
    ];

    for neighbor in directions {
        if is_walkable(ctx, neighbor) {
            let cost = move_cost(pos, neighbor);
            neighbors.push((neighbor, cost));
        }
    }

    neighbors
}

/// Check if a position is walkable.
fn is_walkable(ctx: &PathfindingContext, pos: BlockPos) -> bool {
    // Check if the block at this position is solid
    if let Some(block_ref) = ctx.chunk_layer.block(pos) {
        if block_ref.state.blocks_motion() {
            return false;
        }
    }

    // Check if there's ground below
    let below = BlockPos::new(pos.x, pos.y - 1, pos.z);
    if let Some(block_ref) = ctx.chunk_layer.block(below) {
        if !block_ref.state.blocks_motion() && !ctx.allow_swim {
            return false;
        }
    }

    true
}

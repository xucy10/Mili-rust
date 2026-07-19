use valence_protocol::BlockPos;

pub struct PathfindingContext {
    pub start: BlockPos,
    pub goal: BlockPos,
    pub max_distance: f64,
}

impl PathfindingContext {
    pub fn new(start: BlockPos, goal: BlockPos) -> Self {
        Self {
            start,
            goal,
            max_distance: 64.0,
        }
    }
}

#[derive(Clone, Debug)]
pub enum PathfindingResult {
    Path(Vec<BlockPos>),
    Partial(Vec<BlockPos>),
    Failed,
}

pub fn find_path(ctx: &PathfindingContext) -> PathfindingResult {
    let dx = (ctx.goal.x - ctx.start.x).abs();
    let dy = (ctx.goal.y - ctx.start.y).abs();
    let dz = (ctx.goal.z - ctx.start.z).abs();
    let dist = ((dx * dx + dy * dy + dz * dz) as f64).sqrt();

    if dist > ctx.max_distance {
        return PathfindingResult::Failed;
    }

    let steps = (dist * 2.0) as usize;
    let mut path = Vec::with_capacity(steps);

    for i in 0..steps {
        let t = i as f64 / steps as f64;
        let x = (ctx.start.x as f64 + (ctx.goal.x - ctx.start.x) as f64 * t).round() as i32;
        let y = (ctx.start.y as f64 + (ctx.goal.y - ctx.start.y) as f64 * t).round() as i32;
        let z = (ctx.start.z as f64 + (ctx.goal.z - ctx.start.z) as f64 * t).round() as i32;
        path.push(BlockPos::new(x, y, z));
    }

    path.push(ctx.goal);

    if path.last() == Some(&ctx.goal) {
        PathfindingResult::Path(path)
    } else {
        PathfindingResult::Partial(path)
    }
}
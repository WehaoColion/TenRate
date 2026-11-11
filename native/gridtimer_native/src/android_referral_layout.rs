// v2.13.5.1 - Compact referral forests without rearranging general-purpose cards.
use super::{coordinate, is_referral, Canvas};
use std::collections::{HashMap, HashSet};

const SIBLING_GAP: f32 = 52.0;
const LEVEL_GAP: f32 = 80.0;
const GROUP_GAP: f32 = 116.0;
const ROW_GAP: f32 = 148.0;
const ISOLATED_GAP: f32 = 172.0;
const CONNECTED_ROW_WIDTH: f32 = 1540.0;
const ISOLATED_ROW_WIDTH: f32 = 1080.0;
const FOREIGN_CARD_GAP: f32 = 144.0;

struct Layout {
    placements: Vec<(usize, f32, f32)>,
    first_root: usize,
    max_right: f32,
}

fn layout(canvas: &Canvas) -> Result<Option<Layout>, String> {
    let clients: HashSet<&str> = canvas.nodes.iter()
        .filter(|node| node.kind == "client")
        .map(|node| node.id.as_str())
        .collect();
    if clients.is_empty() {
        return Ok(None);
    }

    let index: HashMap<&str, usize> = canvas.nodes.iter().enumerate()
        .filter(|(_, node)| node.kind == "client")
        .map(|(i, node)| (node.id.as_str(), i))
        .collect();
    let count = canvas.nodes.len();
    let mut children = vec![Vec::<usize>::new(); count];
    let mut has_parent = vec![false; count];
    for edge in canvas.edges.iter().filter(|edge| is_referral(edge, &clients)) {
        let parent = *index.get(edge.from.as_str()).ok_or("推荐人不存在")?;
        let child = *index.get(edge.to.as_str()).ok_or("被推荐客户不存在")?;
        if has_parent[child] {
            return Err("同一客户不能有多个直接推荐人".into());
        }
        children[parent].push(child);
        has_parent[child] = true;
    }

    // Roots and siblings follow the existing card order. A repeated arrange
    // therefore has stable coordinates, even after the user manually drags cards.
    let roots: Vec<usize> = canvas.nodes.iter().enumerate()
        .filter(|(i, node)| node.kind == "client" && !has_parent[*i])
        .map(|(i, _)| i)
        .collect();
    let mut order = Vec::with_capacity(clients.len());
    let mut depths = vec![0usize; count];
    let mut tallest = Vec::<f32>::new();
    let mut seen = HashSet::new();
    let mut stack: Vec<(usize, usize)> = roots.iter().rev().map(|&i| (i, 0)).collect();
    while let Some((i, depth)) = stack.pop() {
        if !seen.insert(i) {
            return Err("推荐关系不能形成循环".into());
        }
        order.push(i);
        depths[i] = depth;
        if depth == tallest.len() {
            tallest.push(0.0);
        }
        tallest[depth] = tallest[depth].max(canvas.nodes[i].height);
        for &child in children[i].iter().rev() {
            stack.push((child, depth + 1));
        }
    }
    if order.len() != clients.len() {
        return Err("推荐关系不能形成循环".into());
    }

    let mut layer_y = vec![0.0; tallest.len()];
    for depth in 1..tallest.len() {
        layer_y[depth] = layer_y[depth - 1] + tallest[depth - 1] + LEVEL_GAP;
    }
    let mut widths = vec![0.0; count];
    let mut spans = vec![0.0; count];
    let mut extents = vec![0.0; count];
    for &i in order.iter().rev() {
        let kids = &children[i];
        let child_span = if kids.is_empty() { 0.0 } else {
            kids.iter().map(|&child| widths[child]).sum::<f32>()
                + (kids.len() - 1) as f32 * SIBLING_GAP
        };
        spans[i] = child_span;
        widths[i] = canvas.nodes[i].width.max(child_span);
        extents[i] = kids.iter()
            .fold(layer_y[depths[i]] + canvas.nodes[i].height,
                  |max_y, &child| max_y.max(extents[child]));
    }

    let mut placements = Vec::with_capacity(clients.len());
    let mut row_x = 0.0f32;
    let mut row_y = 0.0f32;
    let mut row_height = 0.0f32;
    let mut max_right = 0.0f32;
    let connected: Vec<usize> = roots.iter().copied()
        .filter(|&i| !children[i].is_empty())
        .collect();
    let isolated: Vec<usize> = roots.iter().copied()
        .filter(|&i| children[i].is_empty())
        .collect();

    for &root in &connected {
        let width = widths[root];
        let left = if row_x > 0.0 { row_x + GROUP_GAP } else { 0.0 };
        if row_x > 0.0 && left + width > CONNECTED_ROW_WIDTH {
            row_y += row_height + ROW_GAP;
            row_x = 0.0;
            row_height = 0.0;
        }
        let base_x = if row_x > 0.0 { row_x + GROUP_GAP } else { 0.0 };
        let mut pending = vec![(root, base_x)];
        while let Some((i, x)) = pending.pop() {
            let node = &canvas.nodes[i];
            let node_x = x + (widths[i] - node.width) / 2.0;
            let node_y = row_y + layer_y[depths[i]];
            placements.push((i, node_x, node_y));
            max_right = max_right.max(node_x + node.width);
            if !children[i].is_empty() {
                let mut child_x = x + (widths[i] - spans[i]) / 2.0;
                let mut next = Vec::with_capacity(children[i].len());
                for &child in &children[i] {
                    next.push((child, child_x));
                    child_x += widths[child] + SIBLING_GAP;
                }
                pending.extend(next.into_iter().rev());
            }
        }
        row_x = base_x + width;
        row_height = row_height.max(extents[root]);
    }

    // Singletons are not artificially presented as descendants. They occupy a
    // separate compact section below all connected components.
    if !connected.is_empty() && !isolated.is_empty() {
        row_y += row_height + ISOLATED_GAP;
    }
    row_x = 0.0;
    let mut isolated_height = 0.0f32;
    for &i in &isolated {
        let node = &canvas.nodes[i];
        let left = if row_x > 0.0 { row_x + SIBLING_GAP } else { 0.0 };
        if row_x > 0.0 && left + node.width > ISOLATED_ROW_WIDTH {
            row_y += isolated_height + LEVEL_GAP;
            row_x = 0.0;
            isolated_height = 0.0;
        }
        let x = if row_x > 0.0 { row_x + SIBLING_GAP } else { 0.0 };
        placements.push((i, x, row_y));
        row_x = x + node.width;
        isolated_height = isolated_height.max(node.height);
        max_right = max_right.max(x + node.width);
    }
    Ok(Some(Layout { placements, first_root: connected.first()
        .or_else(|| isolated.first()).copied().ok_or("没有客户卡片")?, max_right }))
}

/// Only the explicit customer-tree action invokes this layout. Non-client
/// notes/pages keep their coordinates, ordinary links keep their semantics,
/// and the outer canvas transaction remains undoable as a single action.
pub(super) fn arrange(canvas: &mut Canvas) -> Result<(), String> {
    let Some(mut result) = layout(canvas)? else { return Ok(()); };
    let foreign: Vec<_> = canvas.nodes.iter().filter(|node| node.kind != "client").collect();
    let shift_x = if foreign.is_empty() {
        0.0
    } else {
        let right = foreign.iter().map(|node| node.x + node.width)
            .fold(f32::NEG_INFINITY, f32::max);
        let left = foreign.iter().map(|node| node.x)
            .fold(f32::INFINITY, f32::min);
        if coordinate(right + FOREIGN_CARD_GAP + result.max_right) {
            right + FOREIGN_CARD_GAP
        } else if coordinate(left - FOREIGN_CARD_GAP - result.max_right) {
            left - FOREIGN_CARD_GAP - result.max_right
        } else {
            return Err("其他卡片占用画布边界，无法安全排列客户".into());
        }
    };
    for (_, x, y) in &mut result.placements {
        *x += shift_x;
        if !coordinate(*x) || !coordinate(*y) {
            return Err("推荐关系过大，无法在画布范围内完整排列".into());
        }
    }
    for &(i, x, y) in &result.placements {
        let node = &canvas.nodes[i];
        if !coordinate(x + node.width) || !coordinate(y + node.height) {
            return Err("推荐关系过大，无法在画布范围内完整排列".into());
        }
    }
    // Show the first connected group on completion rather than leaving the
    // user staring at coordinates from an obsolete grid arrangement.
    let root = result.placements.iter()
        .find(|(i, _, _)| *i == result.first_root)
        .ok_or("客户布局缺少根节点")?;
    let root_node = &canvas.nodes[root.0];
    let focus = (root.1 + root_node.width / 2.0, root.2 + root_node.height / 2.0);
    if !coordinate(focus.0) || !coordinate(focus.1) {
        return Err("画布定位超出边界".into());
    }
    for (i, x, y) in result.placements {
        canvas.nodes[i].x = x;
        canvas.nodes[i].y = y;
    }
    canvas.center_x = focus.0;
    canvas.center_y = focus.1;
    Ok(())
}

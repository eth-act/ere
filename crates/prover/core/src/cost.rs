use std::{
    cmp::Reverse,
    collections::{BTreeMap, BTreeSet, HashMap},
    io::Write,
    iter,
    ops::Range,
    sync::Arc,
};

use elf::{
    ElfBytes,
    abi::{PT_LOAD, STB_GLOBAL, STT_FUNC},
    endian::AnyEndian,
};
use flate2::{Compression, write::GzEncoder};
use prost::Message;

use crate::error::CommonError;

#[rustfmt::skip]
pub mod pprof;

/// Sample type of the number of times that a call path is entered.
const CALLS: &str = "calls";

/// Sample type of the heap bytes that a call path reads or writes first.
const HEAP_GROWTH: &str = "heap_growth";

/// Sample type of the sum of the components. The sample type of the component `x` is `cost.x`.
const COST: &str = "cost";

/// Comments of a profile, which describe its sample types.
const COMMENTS: [&str; 3] = [
    "calls is the number of times that the call path is entered by a call, or by a jump or fall-through from another function. A return to the caller does not count.",
    "heap_growth is the bytes of heap that the call path reads or writes first, outside the loadable ELF segments, the stack and the memory that the zkVM uses itself. Its sum is the peak heap.",
    "cost is the sum of the cost.<component> values, which are the costs of the zkVM components.",
];

/// Node above the frame of the entry point. It has no function and spends no cost.
const ROOT: usize = 0;

/// Frames of a pprof sample at most. A deeper stack keeps its innermost frames, which bounds the
/// profile size for deep recursion.
const MAX_STACK_DEPTH: usize = 1024;

/// Frame of the code outside every function symbol.
const UNKNOWN_FRAME: &str = "[unknown]";

/// Bytes of memory that one page of [`PeakMemory`] covers.
const PAGE_BYTES: u64 = 1 << 16;

/// Doublewords of memory that a page covers.
const PAGE_DOUBLEWORDS: usize = (PAGE_BYTES / 8) as usize;

/// Most that one `addi` lowers a register by, because its 12-bit signed immediate is at least
/// -2,048.
const ADDI_MAX_DECREMENT: u64 = 1 << 11;

/// Cost of one execution.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CostEstimation {
    /// Cost per component. Each zkVM defines the unit.
    pub cost: BTreeMap<String, u64>,
}

/// Cost of one execution per guest call stack, and its peak memory use.
#[derive(Clone, Debug, PartialEq)]
pub struct CostProfile {
    /// A sample has one value per sample type. The types are `calls`, `heap_growth`, `cost` and
    /// one `cost.<component>` per [`CostEstimation::cost`] component, and the profile comments
    /// describe them.
    pub pprof: pprof::Profile,
    /// Span of the stack pointer `x2`, as [`PeakMemory::stack_pointer`] counts it.
    pub peak_stack_bytes: u64,
    /// Bytes of heap that the guest reads or writes, as [`PeakMemory::peak_heap_bytes`] counts
    /// them. OpenVM counts whole 16-byte leaves.
    pub peak_heap_bytes: u64,
}

/// Guest call tree of one execution, with the cost per component that each frame spends itself.
///
/// A profiler steps the guest and, at each frame change, charges the cost growth since the last
/// change to the frame that ran. Frames are the sized function symbols of the ELF, and code outside
/// every symbol shows as `[unknown]`. [`Self::transfer`] decides the frame changes.
pub struct CallTree {
    symbol_map: Arc<SymbolMap>,
    /// Names of the root frames that [`Self::charge_root`] charges. Their name indices follow the
    /// function names.
    roots: Vec<String>,
    components: Vec<String>,
    nodes: Vec<Node>,
    children: HashMap<(usize, usize), usize>,
    /// Cost of each node, one value per component.
    costs: Vec<u64>,
    /// Running cost at the last charge.
    charged: Vec<u64>,
    /// Node of the frame that runs.
    node: usize,
    /// Address range of the function of `node`.
    range: Range<u64>,
    /// Calls on the stack, outermost first.
    calls: Vec<Call>,
}

/// Frame of a [`CallTree`] that runs, which [`PeakMemory::access`] records. The root node is never
/// a frame that runs, so 0 is no frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame(u32);

/// Action of a jump on the return-address stack, from the return-address stack prediction hints of
/// the RISC-V unprivileged ISA. The link registers are `ra` and `t0`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasAction {
    Push,
    Pop,
    PopThenPush,
}

/// Map from guest addresses to function symbols, as parts that cover the full address space.
pub struct SymbolMap {
    /// Start address and name index of each part, in address order. A part ends where the next
    /// part starts.
    parts: Vec<(u64, usize)>,
    /// Distinct demangled names. Index 0 is [`UNKNOWN_FRAME`], the name of each gap between
    /// functions.
    names: Vec<String>,
}

/// Peak stack and heap use of one execution, from the stack pointer and the memory accesses.
pub struct PeakMemory {
    memory: Range<u64>,
    /// Address ranges that hold no heap.
    non_heap: Vec<Range<u64>>,
    /// Frame of the first read or write of each doubleword of `memory`, or 0 for a doubleword that
    /// no frame touches, in pages that the first access allocates.
    pages: Vec<Option<Box<[u32; PAGE_DOUBLEWORDS]>>>,
    sp: u64,
    /// Whether the step that wrote `sp` wrote an upper immediate.
    upper_immediate: bool,
    /// Run of stack pointers that ends at `sp`.
    run: StackRun,
    stack_top: u64,
    stack_bottom: u64,
}

/// How a step wrote the stack pointer `x2`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StackPointerWrite {
    /// The step loaded `x2` from memory.
    Load,
    /// The step wrote an upper immediate with `lui` or `auipc`.
    UpperImmediate,
    /// The step wrote another value to `x2`, or did not write it.
    Other,
}

/// Frame of a call stack in a [`CallTree`].
#[derive(Clone, Copy)]
struct Node {
    /// Node of the caller.
    parent: usize,
    /// Name index of the function.
    name: usize,
    /// Number of times that the frame is entered.
    entries: u64,
}

/// Call on the stack of a [`CallTree`].
#[derive(Clone, Copy, PartialEq, Eq)]
struct Call {
    return_address: u64,
    /// Stack pointer at the call. A call into code outside every symbol has none, because it can
    /// switch stacks.
    stack_pointer: Option<u64>,
}

/// Run of stack pointer values in [`PeakMemory`].
#[derive(Clone, Copy, PartialEq, Eq)]
struct StackRun {
    first: u64,
    lowest: u64,
    /// Whether a step kept one of the values.
    kept: bool,
}

impl CostProfile {
    /// Cost per component, equal to the `execute_estimated_cost` result of the same input.
    pub fn cost_estimation(&self) -> CostEstimation {
        let cost = self
            .pprof
            .sample_type
            .iter()
            .enumerate()
            .filter_map(|(index, sample_type)| {
                let component = self.pprof.string_table[sample_type.r#type as usize]
                    .strip_prefix(COST)?
                    .strip_prefix('.')?;
                let cost = self
                    .pprof
                    .sample
                    .iter()
                    .map(|sample| sample.value[index] as u64)
                    .sum();
                Some((component.to_owned(), cost))
            })
            .collect();
        CostEstimation { cost }
    }

    /// The gzipped pprof file that `go tool pprof` reads.
    pub fn to_pb_gz(&self) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&self.to_pb()).unwrap();
        encoder.finish().unwrap()
    }

    /// The pprof protobuf that [`Self::to_pb_gz`] gzips.
    pub fn to_pb(&self) -> Vec<u8> {
        self.pprof.encode_to_vec()
    }
}

impl CallTree {
    /// Call tree over `symbol_map` that starts in the frame of the function at `pc_start`.
    pub fn new(symbol_map: Arc<SymbolMap>, pc_start: u64, components: &[&str]) -> Self {
        let mut call_tree = Self {
            symbol_map,
            roots: Vec::new(),
            components: components.iter().map(|name| (*name).to_owned()).collect(),
            nodes: vec![Node {
                parent: ROOT,
                name: 0,
                entries: 0,
            }],
            children: HashMap::new(),
            costs: vec![0; components.len()],
            charged: vec![0; components.len()],
            node: ROOT,
            range: 0..0,
            calls: Vec::new(),
        };
        call_tree.enter(ROOT, pc_start);
        call_tree
    }

    /// Whether `pc` is in the function of the frame that runs.
    #[inline]
    pub fn contains(&self, pc: u64) -> bool {
        self.range.contains(&pc)
    }

    /// The frame that runs.
    #[inline]
    pub fn frame(&self) -> Frame {
        Frame(u32::try_from(self.node).unwrap())
    }

    /// Charges the growth of `running`, the cost per component so far, to the frame that runs.
    pub fn charge(&mut self, running: &[u64]) {
        self.charge_node(self.node, running);
    }

    /// Charges the growth of `running` to the root frame `name`, which has no function symbol.
    pub fn charge_root(&mut self, name: &str, running: &[u64]) {
        let root = self
            .roots
            .iter()
            .position(|root| root == name)
            .unwrap_or_else(|| {
                self.roots.push(name.to_owned());
                self.roots.len() - 1
            });
        let node = self.child(ROOT, self.symbol_map.names.len() + root);
        self.charge_node(node, running);
    }

    /// Moves to the frame that runs at `target`, the PC after a jump or the first PC outside the
    /// function. `return_address` is the value that the jump writes to its link register, and `sp`
    /// is the stack pointer at the jump. `action` follows the return-address stack prediction hints
    /// of `jalr` in the RISC-V unprivileged ISA.
    ///
    /// - A return goes to the innermost call with the same return address and stack pointer. A call
    ///   to the return address of the running frame also returns, as `jalr ra, 0(ra)` does.
    /// - Other jumps to a function start enter a frame when they link `ra` or `t0`, and replace it
    ///   otherwise, because NativeAOT jumps through `t0` as a scratch register.
    /// - A pop-then-push jump resumes the call whose return address it targets, as a coroutine swap
    ///   does. Otherwise it calls into code outside every symbol, or replaces the frame.
    /// - A return that matches no call keeps a frame whose function holds the target, as a jump
    ///   table does. Otherwise it unwinds to the caller that `nonlocal_jump_frames` finds, or
    ///   replaces the caller.
    /// - Any other jump out of the function replaces the frame, as a tail call does.
    pub fn transfer(
        &mut self,
        action: Option<RasAction>,
        target: u64,
        return_address: u64,
        sp: u64,
    ) {
        let parent = self.nodes[self.node].parent;
        let returned = match action {
            Some(RasAction::Pop) => self.calls.iter().rposition(|call| {
                call.return_address == target
                    && call.stack_pointer.is_none_or(|call_sp| call_sp == sp)
            }),
            Some(RasAction::Push) => {
                let call = Call {
                    return_address: target,
                    stack_pointer: Some(sp),
                };
                (self.calls.last() == Some(&call)).then(|| self.calls.len() - 1)
            }
            _ => None,
        };
        let action = match action {
            _ if returned.is_some() => Some(RasAction::Pop),
            Some(RasAction::PopThenPush) if self.symbol_map.is_start(target) => {
                Some(RasAction::Push)
            }
            Some(RasAction::Pop) if self.symbol_map.is_start(target) => None,
            action => action,
        };
        match action {
            Some(RasAction::Push) => self.call(target, return_address, Some(sp)),
            Some(RasAction::Pop) if parent != ROOT => {
                let (name, range) = self.symbol_map.lookup(target);
                if let Some(index) = returned {
                    self.unwind(self.calls.len() - index, range);
                } else if !self.contains(target) {
                    match self.nonlocal_jump_frames(name, sp) {
                        Some(frames) => self.unwind(frames, range),
                        None => {
                            // A return into a function that did not make the call replaces the
                            // caller.
                            self.calls.pop();
                            self.enter(self.nodes[parent].parent, target);
                        }
                    }
                }
            }
            Some(RasAction::PopThenPush) => {
                // Each coroutine has a stack of its own, so only the address has to match.
                let resumed = self
                    .calls
                    .iter()
                    .rposition(|call| call.return_address == target);
                if let Some(index) = resumed {
                    self.unwind(self.calls.len() - index, self.symbol_map.lookup(target).1);
                } else if self.symbol_map.is_gap(target) {
                    self.call(target, return_address, None);
                } else {
                    self.enter(parent, target);
                }
            }
            _ => self.enter(parent, target),
        }
    }

    /// Profile in `unit` with one sample per call path that is entered, spends cost or grows the
    /// heap. A stack deeper than `MAX_STACK_DEPTH` frames keeps its innermost ones, and equal
    /// stacks then share a sample.
    pub fn into_profile(self, unit: &str, memory: &PeakMemory) -> CostProfile {
        let mut string_table = vec![String::new()];
        let mut intern = |string: &str| {
            let index = string_table
                .iter()
                .position(|entry| entry == string)
                .unwrap_or_else(|| {
                    string_table.push(string.to_owned());
                    string_table.len() - 1
                });
            index as i64
        };
        let mut value_type = |r#type: &str, unit: &str| pprof::ValueType {
            r#type: intern(r#type),
            unit: intern(unit),
        };
        let mut sample_type = vec![
            value_type(CALLS, "count"),
            value_type(HEAP_GROWTH, "bytes"),
            value_type(COST, unit),
        ];
        for component in &self.components {
            sample_type.push(value_type(&format!("{COST}.{component}"), unit));
        }
        let cost = sample_type[2];
        let comment = COMMENTS.map(&mut intern).to_vec();

        let heap_growth = memory.heap_growth();
        let peak_heap_bytes = heap_growth.iter().sum();
        // The values of a node follow the order of `sample_type`.
        let values = self
            .nodes
            .iter()
            .zip(self.costs.chunks_exact(self.components.len()))
            .enumerate()
            .skip(ROOT + 1)
            .map(|(node, (Node { entries, .. }, costs))| {
                let heap = heap_growth.get(node).copied().unwrap_or(0);
                let values: Vec<u64> = [*entries, heap, costs.iter().sum()]
                    .into_iter()
                    .chain(costs.iter().copied())
                    .collect();
                (node, values)
            })
            .filter(|(_, values)| values.iter().any(|value| *value != 0));

        // Each frame name gets one function and one location, whose id is their position in
        // `function` and `location` plus one.
        let mut ids = HashMap::new();
        let mut samples = HashMap::new();
        let (mut function, mut location, mut sample) = (Vec::new(), Vec::new(), Vec::new());
        for (node, values) in values {
            let location_id: Vec<u64> = self
                .stack(node)
                .take(MAX_STACK_DEPTH)
                .map(|frame| {
                    let name = self.nodes[frame].name;
                    *ids.entry(name).or_insert_with(|| {
                        string_table.push(self.name(name).to_owned());
                        let id = function.len() as u64 + 1;
                        function.push(pprof::Function {
                            id,
                            name: string_table.len() as i64 - 1,
                            ..Default::default()
                        });
                        location.push(pprof::Location {
                            id,
                            mapping_id: 1,
                            line: vec![pprof::Line {
                                function_id: id,
                                ..Default::default()
                            }],
                            ..Default::default()
                        });
                        id
                    })
                })
                .collect();
            // Only stacks at the depth limit can be equal, and then they share a sample.
            let index = *samples
                .entry(location_id)
                .or_insert_with_key(|location_id| {
                    sample.push(pprof::Sample {
                        location_id: location_id.clone(),
                        value: vec![0; values.len()],
                        label: Vec::new(),
                    });
                    sample.len() - 1
                });
            for (sum, value) in sample[index].value.iter_mut().zip(values) {
                *sum += i64::try_from(value).unwrap();
            }
        }

        CostProfile {
            pprof: pprof::Profile {
                sample_type,
                sample,
                mapping: vec![pprof::Mapping {
                    id: 1,
                    has_functions: true,
                    ..Default::default()
                }],
                location,
                function,
                string_table,
                period_type: Some(cost),
                period: 1,
                comment,
                default_sample_type: cost.r#type,
                ..Default::default()
            },
            peak_stack_bytes: memory.peak_stack_bytes(),
            peak_heap_bytes,
        }
    }

    fn enter(&mut self, parent: usize, pc: u64) {
        let (name, range) = self.symbol_map.lookup(pc);
        self.node = self.child(parent, name);
        self.nodes[self.node].entries += 1;
        self.range = range;
    }

    fn child(&mut self, parent: usize, name: usize) -> usize {
        *self.children.entry((parent, name)).or_insert_with(|| {
            self.nodes.push(Node {
                parent,
                name,
                entries: 0,
            });
            self.costs.extend(iter::repeat_n(0, self.components.len()));
            self.nodes.len() - 1
        })
    }

    fn charge_node(&mut self, node: usize, running: &[u64]) {
        let costs = &mut self.costs[node * self.components.len()..][..self.components.len()];
        for ((cost, charged), running) in costs.iter_mut().zip(&mut self.charged).zip(running) {
            *cost += running
                .checked_sub(*charged)
                .expect("the running cost only grows");
            *charged = *running;
        }
    }

    /// Enters the frame of a call to `pc` that returns to `return_address` with the stack pointer
    /// `sp`, if known.
    fn call(&mut self, pc: u64, return_address: u64, sp: Option<u64>) {
        self.enter(self.node, pc);
        self.calls.push(Call {
            return_address,
            stack_pointer: sp,
        });
    }

    /// Returns from `frames` frames into the function whose address range is `range`.
    fn unwind(&mut self, frames: usize, range: Range<u64>) {
        self.node = (0..frames).fold(self.node, |node, _| self.nodes[node].parent);
        let depth = self.calls.len() - frames;
        self.calls.truncate(depth);
        self.range = range;
    }

    /// Frames that a nonlocal jump such as `longjmp` to the function `name` with the stack
    /// pointer `sp` leaves. It returns to the innermost caller in `name` whose call ran with a
    /// higher stack pointer, else to the innermost caller in `name`.
    fn nonlocal_jump_frames(&self, name: usize, sp: u64) -> Option<usize> {
        // The first frame has no call, so it holds every stack pointer.
        let call_sp = |frames: usize| {
            self.calls
                .iter()
                .rev()
                .nth(frames)
                .and_then(|call| call.stack_pointer)
                .unwrap_or(u64::MAX)
        };
        let mut callers = self
            .stack(self.node)
            .enumerate()
            .skip(1)
            .filter(|&(_, node)| self.nodes[node].name == name)
            .map(|(frames, _)| frames);
        callers
            .clone()
            .find(|&frames| call_sp(frames) > sp)
            .or_else(|| callers.next())
    }

    /// Nodes of the call stack of `node`, innermost first.
    fn stack(&self, node: usize) -> impl Iterator<Item = usize> + Clone + '_ {
        iter::successors(Some(node), |&node| Some(self.nodes[node].parent))
            .take_while(|&node| node != ROOT)
    }

    /// Name of the function or root frame with the name index `name`.
    fn name(&self, name: usize) -> &str {
        self.symbol_map
            .names
            .get(name)
            .unwrap_or_else(|| &self.roots[name - self.symbol_map.names.len()])
    }
}

impl RasAction {
    /// Action of a `jal` or `jalr` that writes `rd` and jumps through `rs1`. A `jal` jumps through
    /// no register, so it passes its `rd` as `rs1`.
    pub fn from_jump(rd: u8, rs1: u8) -> Option<Self> {
        let is_link = |register| matches!(register, 1 | 5);
        match (is_link(rd), is_link(rs1)) {
            (true, true) if rd != rs1 => Some(Self::PopThenPush),
            (true, _) => Some(Self::Push),
            (false, true) => Some(Self::Pop),
            (false, false) => None,
        }
    }
}

impl SymbolMap {
    /// Symbol map of the sized function symbols of `elf`.
    pub fn from_elf(elf: &[u8]) -> Result<Self, CommonError> {
        let elf = ElfBytes::<AnyEndian>::minimal_parse(elf)
            .map_err(|err| CommonError::deserialize("ELF symbols", "elf", err))?;
        // A symbol table or a name that does not parse only loses its frame names.
        let Ok(Some((symbols, strings))) = elf.symbol_table() else {
            return Ok(Self::new(Vec::new()));
        };
        let symbols = symbols
            .iter()
            .filter(|symbol| symbol.st_symtype() == STT_FUNC && symbol.st_size > 0)
            .filter_map(|symbol| {
                let name = strings.get(symbol.st_name as usize).ok()?;
                Some((
                    symbol.st_value,
                    symbol.st_value.checked_add(symbol.st_size)?,
                    symbol.st_bind() == STB_GLOBAL,
                    name,
                ))
            })
            .collect();
        Ok(Self::new(symbols))
    }

    /// Start of each function and of each gap between functions, in address order.
    pub fn starts(&self) -> impl Iterator<Item = u64> + '_ {
        self.parts.iter().map(|(start, _)| *start)
    }

    /// Symbol map of `symbols`, each a start, an end, whether it is global, and a name.
    fn new(mut symbols: Vec<(u64, u64, bool, &str)>) -> Self {
        // An enclosing symbol sorts before a symbol that starts with it, and a global symbol before
        // its local and weak aliases, as perf prefers it.
        symbols.sort_unstable_by_key(|&(start, end, global, name)| {
            (start, Reverse(end), Reverse(global), name)
        });
        // An alias, or a symbol inside the previous function, adds no function.
        let functions: Vec<(u64, u64, String)> = symbols
            .into_iter()
            .scan(0, |covered, (start, end, _, name)| {
                let kept = start >= *covered;
                if kept {
                    *covered = end;
                }
                Some(kept.then(|| (start, end, format!("{:#}", rustc_demangle::demangle(name)))))
            })
            .flatten()
            .collect();
        // Functions of one name, such as copies of one generic function from several crates, share
        // one frame, as pprof and FlameGraph merge frames by name.
        let names: Vec<String> = iter::once(UNKNOWN_FRAME)
            .chain(
                functions
                    .iter()
                    .map(|(_, _, name)| name.as_str())
                    .collect::<BTreeSet<_>>(),
            )
            .map(str::to_owned)
            .collect();
        let name_index = |name: &String| names[1..].binary_search(name).unwrap() + 1;
        // A gap starts at 0 and at the end of each function, and a function that starts where a
        // gap would start replaces it.
        let parts: BTreeMap<u64, usize> = iter::once((0, 0))
            .chain(functions.iter().map(|(_, end, _)| (*end, 0)))
            .chain(
                functions
                    .iter()
                    .map(|(start, _, name)| (*start, name_index(name))),
            )
            .collect();
        Self {
            parts: parts.into_iter().collect(),
            names,
        }
    }

    /// Whether `pc` is the start of a function.
    fn is_start(&self, pc: u64) -> bool {
        let (name, range) = self.lookup(pc);
        name != 0 && range.start == pc
    }

    /// Whether `pc` is outside every function symbol.
    fn is_gap(&self, pc: u64) -> bool {
        self.lookup(pc).0 == 0
    }

    /// Name index and address range of the part that holds `pc`.
    fn lookup(&self, pc: u64) -> (usize, Range<u64>) {
        let part = self.parts.partition_point(|(start, _)| *start <= pc) - 1;
        let end = self
            .parts
            .get(part + 1)
            .map_or(u64::MAX, |(start, _)| *start);
        (self.parts[part].1, self.parts[part].0..end)
    }
}

impl PeakMemory {
    /// Tracks accesses to the zkVM memory in `memory`. `non_heap` holds the address ranges that
    /// hold no heap, the loadable segments of the guest ELF and the memory that the executor uses.
    pub fn new(memory: Range<u64>, non_heap: &[Range<u64>]) -> Self {
        let pages = (memory.end - memory.start).div_ceil(PAGE_BYTES) as usize;
        Self {
            memory,
            non_heap: non_heap.to_vec(),
            pages: vec![None; pages],
            sp: 0,
            upper_immediate: false,
            run: StackRun::new(0),
            stack_top: 0,
            stack_bottom: u64::MAX,
        }
    }

    /// Records a read or write of `len` bytes at `address` by `frame`.
    #[inline]
    pub fn access(&mut self, address: u64, len: u64, frame: Frame) {
        let start = address.max(self.memory.start);
        let end = address.saturating_add(len).min(self.memory.end);
        if start >= end {
            return;
        }
        let doublewords_per_page = PAGE_DOUBLEWORDS as u64;
        for doubleword in (start - self.memory.start) / 8..=(end - 1 - self.memory.start) / 8 {
            let page = self.pages[(doubleword / doublewords_per_page) as usize]
                .get_or_insert_with(new_page);
            let first = &mut page[(doubleword % doublewords_per_page) as usize];
            if *first == 0 {
                *first = frame.0;
            }
        }
    }

    /// Records the stack pointer after a step, and how the step wrote it. The peak stack is the
    /// span of the counted runs of values.
    ///
    /// - A lower value that is no load and no upper immediate continues the run. Any other new
    ///   value starts a run, and zero never counts.
    /// - A run counts from its first to its lowest value once a step keeps one of its values.
    /// - An upper immediate that the next step lowers by no more than one `addi` can is part of an
    ///   address, as in `auipc sp` and `addi sp, sp, -8`, so its run starts at the lower value.
    #[inline]
    pub fn stack_pointer(&mut self, sp: u64, write: StackPointerWrite) {
        if sp == self.sp {
            self.run.kept |= sp != 0;
        } else if sp > self.sp || write != StackPointerWrite::Other || sp == 0 {
            (self.stack_top, self.stack_bottom) = self.stack_span();
            self.run = StackRun::new(sp);
        } else if self.upper_immediate
            && self.run == StackRun::new(self.sp)
            // `addiw` completes the sign-extended value of a `lui` in the low 32 bits.
            && u64::from((self.sp as u32).wrapping_sub(sp as u32)) <= ADDI_MAX_DECREMENT
        {
            self.run = StackRun::new(sp);
        } else {
            self.run.lowest = sp;
        }
        self.upper_immediate = write == StackPointerWrite::UpperImmediate;
        self.sp = sp;
    }

    pub fn peak_stack_bytes(&self) -> u64 {
        let (top, bottom) = self.stack_span();
        top.saturating_sub(bottom)
    }

    /// Bytes of the doublewords that the guest reads or writes and that overlap neither a
    /// non-heap range nor the stack span. The heap of a guest lies there, whatever its runtime.
    pub fn peak_heap_bytes(&self) -> u64 {
        self.heap_growth().iter().sum()
    }

    /// Highest and lowest stack pointer with the run that ends at `sp`, when a step kept one of its
    /// values.
    fn stack_span(&self) -> (u64, u64) {
        if self.run.kept {
            (
                self.stack_top.max(self.run.first),
                self.stack_bottom.min(self.run.lowest),
            )
        } else {
            (self.stack_top, self.stack_bottom)
        }
    }

    /// Bytes of the heap doublewords of [`Self::peak_heap_bytes`] per frame of their first read or
    /// write, indexed by the frame.
    fn heap_growth(&self) -> Vec<u64> {
        let (top, bottom) = self.stack_span();
        let excluded: Vec<Range<u64>> = self
            .non_heap
            .iter()
            .cloned()
            .chain(iter::once(bottom..top))
            .collect();
        let overlaps = |start: u64, len: u64| {
            excluded
                .iter()
                .any(|range| range.start < start + len && start < range.end)
        };
        let mut growth = Vec::new();
        let pages = self.pages.iter().enumerate().filter_map(|(page, frames)| {
            Some((
                self.memory.start + page as u64 * PAGE_BYTES,
                frames.as_deref()?,
            ))
        });
        for (page_start, frames) in pages {
            // Only a page that overlaps an excluded range needs a check per doubleword.
            let partial = overlaps(page_start, PAGE_BYTES);
            for (doubleword, &frame) in frames.iter().enumerate() {
                if frame != 0 && !(partial && overlaps(page_start + 8 * doubleword as u64, 8)) {
                    let frame = frame as usize;
                    if frame >= growth.len() {
                        growth.resize(frame + 1, 0);
                    }
                    growth[frame] += 8;
                }
            }
        }
        growth
    }
}

/// Address ranges of the `PT_LOAD` segments of `elf`, the memory that the loader fills.
pub fn loadable_segments(elf: &[u8]) -> Result<Vec<Range<u64>>, CommonError> {
    let elf = ElfBytes::<AnyEndian>::minimal_parse(elf)
        .map_err(|err| CommonError::deserialize("ELF segments", "elf", err))?;
    Ok(elf
        .segments()
        .into_iter()
        .flatten()
        .filter(|segment| segment.p_type == PT_LOAD)
        .map(|segment| segment.p_vaddr..segment.p_vaddr.saturating_add(segment.p_memsz))
        .collect())
}

impl StackRun {
    fn new(sp: u64) -> Self {
        Self {
            first: sp,
            lowest: sp,
            kept: false,
        }
    }
}

/// Page of a first access, kept out of the hot path of [`PeakMemory::access`].
#[cold]
fn new_page() -> Box<[u32; PAGE_DOUBLEWORDS]> {
    Box::new([0; PAGE_DOUBLEWORDS])
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, io::Read, sync::Arc};

    use flate2::read::GzDecoder;
    use prost::Message;

    use crate::cost::{
        CallTree, CostEstimation, CostProfile, Frame, MAX_STACK_DEPTH, PeakMemory, RasAction,
        StackPointerWrite::{Load, Other, UpperImmediate},
        SymbolMap, UNKNOWN_FRAME, pprof,
    };

    #[test]
    fn call_tree_splits_cost_entries_and_heap_growth_per_frame() {
        let symbol_map = SymbolMap::new(vec![
            (0x100, 0x200, true, "main"),
            (0x200, 0x300, true, "f"),
            (0x300, 0x400, true, "g"),
        ]);
        let mut call_tree = CallTree::new(Arc::new(symbol_map), 0x100, &["a", "b"]);
        let mut memory = PeakMemory::new(0..0x1000, &[]);
        memory.access(0x800, 16, call_tree.frame());
        call_tree.charge(&[1, 0]);
        call_tree.transfer(RasAction::from_jump(1, 1), 0x200, 0x150, 0x1000);
        // `f` reads a doubleword that `main` touched, and one more.
        memory.access(0x808, 16, call_tree.frame());
        call_tree.charge(&[3, 5]);
        call_tree.transfer(RasAction::from_jump(0, 1), 0x150, 0, 0x1000);
        call_tree.charge(&[4, 5]);
        // `main` tail-calls `g`, which reads only touched doublewords.
        call_tree.transfer(RasAction::from_jump(0, 0), 0x300, 0, 0x1000);
        memory.access(0x800, 24, call_tree.frame());
        call_tree.charge(&[6, 5]);
        call_tree.charge_root("[base]", &[6, 9]);
        let profile = call_tree.into_profile("cells", &memory);

        let string = |index: i64| profile.pprof.string_table[index as usize].as_str();
        let sample_types: Vec<(&str, &str)> = profile
            .pprof
            .sample_type
            .iter()
            .map(|sample_type| (string(sample_type.r#type), string(sample_type.unit)))
            .collect();
        assert_eq!(
            sample_types,
            [
                ("calls", "count"),
                ("heap_growth", "bytes"),
                ("cost", "cells"),
                ("cost.a", "cells"),
                ("cost.b", "cells"),
            ]
        );
        assert_eq!(string(profile.pprof.default_sample_type), "cost");
        assert_eq!(profile.pprof.comment.len(), 3);
        let samples: Vec<(Vec<&str>, &[i64])> = profile
            .pprof
            .sample
            .iter()
            .map(|sample| (frames(&profile, sample), sample.value.as_slice()))
            .collect();
        assert_eq!(
            samples,
            [
                (vec!["main"], &[1, 16, 2, 2, 0][..]),
                (vec!["main", "f"], &[1, 8, 7, 2, 5][..]),
                (vec!["g"], &[1, 0, 2, 2, 0][..]),
                (vec!["[base]"], &[0, 0, 4, 0, 4][..]),
            ]
        );
        assert_eq!(profile.peak_heap_bytes, 24);
        assert_eq!(
            profile.cost_estimation(),
            CostEstimation {
                cost: [("a".to_owned(), 6), ("b".to_owned(), 9)].into(),
            }
        );

        let mut encoded = Vec::new();
        GzDecoder::new(profile.to_pb_gz().as_slice())
            .read_to_end(&mut encoded)
            .unwrap();
        assert_eq!(encoded, profile.to_pb());
        assert_eq!(
            pprof::Profile::decode(encoded.as_slice()).unwrap(),
            profile.pprof
        );
    }

    /// Frames of `sample` from the root.
    fn frames<'a>(profile: &'a CostProfile, sample: &pprof::Sample) -> Vec<&'a str> {
        sample
            .location_id
            .iter()
            .rev()
            .map(|id| {
                let function = &profile.pprof.function[*id as usize - 1];
                profile.pprof.string_table[function.name as usize].as_str()
            })
            .collect()
    }

    /// A jump as the `rd` and `rs1` that [`RasAction::from_jump`] reads.
    type Jump = (u8, u8);
    /// `jal ra` or `jalr ra, X(ra)`, a call.
    const CALL: Jump = (1, 1);
    /// `jalr ra, X(t0)`, which pops `t0` and pushes `ra`.
    const CALL_T0: Jump = (1, 5);
    /// `jalr ra, X(a2)`, a call through a register without a return address.
    const CALL_A2: Jump = (1, 12);
    /// `jalr t0, X(ra)`, which pops `ra` and pushes `t0`.
    const SWAP_RA: Jump = (5, 1);
    /// `ret`, a return through `ra`.
    const RET: Jump = (0, 1);
    /// `jr t0`, a return through `t0`.
    const JR_T0: Jump = (0, 5);
    /// `j`, a jump without a link register.
    const JUMP: Jump = (0, 0);

    /// Runs `jumps` of `(jump, target, return_address, sp)` from `entry` over global `functions`.
    /// The frame that runs before jump `i` spends `cost(i)`, and the frame after the last jump
    /// spends `cost(jumps.len())`.
    fn run(
        functions: Vec<(u64, u64, &str)>,
        entry: u64,
        jumps: &[(Jump, u64, u64, u64)],
        cost: impl Fn(usize) -> u64,
    ) -> CallTree {
        let functions = functions
            .into_iter()
            .map(|(start, end, name)| (start, end, true, name))
            .collect();
        let mut call_tree = CallTree::new(Arc::new(SymbolMap::new(functions)), entry, &["a"]);
        let mut running = 0;
        for (index, &((rd, rs1), target, return_address, sp)) in jumps.iter().enumerate() {
            running += cost(index);
            call_tree.charge(&[running]);
            call_tree.transfer(RasAction::from_jump(rd, rs1), target, return_address, sp);
        }
        call_tree.charge(&[running + cost(jumps.len())]);
        call_tree
    }

    /// Cost of each stack, with its frames from the root joined by `;`.
    fn stacks(call_tree: CallTree) -> BTreeMap<String, u64> {
        let profile = call_tree.into_profile("cells", &PeakMemory::new(0..0, &[]));
        let cost = profile
            .pprof
            .sample_type
            .iter()
            .position(|sample_type| {
                profile.pprof.string_table[sample_type.r#type as usize] == "cost"
            })
            .unwrap();
        profile
            .pprof
            .sample
            .iter()
            .map(|sample| {
                (
                    frames(&profile, sample).join(";"),
                    sample.value[cost] as u64,
                )
            })
            .collect()
    }

    #[test]
    fn call_tree_follows_calls_returns_and_tail_calls() {
        let main_f = || vec![(0x100, 0x200, "main"), (0x200, 0x300, "f")];
        // The frame that runs before jump `i` spends `1 << i`, so the cost of a stack names the
        // segments it ran.
        for (case, functions, entry, jumps, expected) in [
            (
                "`longjmp` past two frames, a jump table in `memset`, and a return into a function \
                 that made no call, with two functions of one name",
                vec![
                    (0x100, 0x200, "main"),
                    (0x200, 0x300, "f"),
                    (0x300, 0x400, "helper"),
                    (0x400, 0x500, "helper"),
                    (0x500, 0x600, "memset"),
                ],
                0x100,
                vec![
                    (CALL, 0x200, 0x104, 0x1000),
                    (CALL, 0x300, 0x204, 0xff0),
                    (CALL, 0x400, 0x304, 0xfe0),
                    (CALL, 0x400, 0x404, 0xfd0),
                    (RET, 0x150, 0, 0x1000),
                    (CALL, 0x500, 0x160, 0x1000),
                    (JR_T0, 0x580, 0, 0x1000),
                    (RET, 0x160, 0, 0x1000),
                    (CALL, 0x200, 0x170, 0x1000),
                    (RET, 0x350, 0, 0x1000),
                ],
                vec![
                    ("main", vec![0, 5, 8]),
                    ("main;f", vec![1, 9]),
                    ("main;f;helper", vec![2]),
                    ("main;f;helper;helper", vec![3]),
                    ("main;f;helper;helper;helper", vec![4]),
                    ("main;memset", vec![6, 7]),
                    ("helper", vec![10]),
                ],
            ),
            (
                "`f` calls itself, jumps through `t0` into its own jump table, and returns twice",
                main_f(),
                0x100,
                vec![
                    (CALL, 0x200, 0x104, 0x1000),
                    (CALL, 0x200, 0x214, 0xff0),
                    (JR_T0, 0x280, 0, 0xfe0),
                    (RET, 0x214, 0, 0xff0),
                    (RET, 0x104, 0, 0x1000),
                ],
                vec![
                    ("main", vec![0, 5]),
                    ("main;f", vec![1, 4]),
                    ("main;f;f", vec![2, 3]),
                ],
            ),
            (
                "the inner `f` jumps through `t0` to the continuation of its own call and returns \
                 there",
                main_f(),
                0x100,
                vec![
                    (CALL, 0x200, 0x104, 0x1000),
                    (CALL, 0x200, 0x214, 0xff0),
                    (JR_T0, 0x214, 0, 0xfe0),
                    (RET, 0x214, 0, 0xff0),
                    (RET, 0x104, 0, 0x1000),
                ],
                vec![
                    ("main", vec![0, 5]),
                    ("main;f", vec![1, 4]),
                    ("main;f;f", vec![2, 3]),
                ],
            ),
            (
                "`g` returns into the outer `f` with its stack pointer, as `longjmp` does",
                vec![
                    (0x100, 0x200, "main"),
                    (0x200, 0x300, "f"),
                    (0x300, 0x400, "g"),
                ],
                0x100,
                vec![
                    (CALL, 0x200, 0x104, 0x1000),
                    (CALL, 0x200, 0x224, 0xff0),
                    (CALL, 0x300, 0x244, 0xfe0),
                    (RET, 0x210, 0, 0xff0),
                    (RET, 0x104, 0, 0x1000),
                ],
                vec![
                    ("main", vec![0, 5]),
                    ("main;f", vec![1, 4]),
                    ("main;f;f", vec![2]),
                    ("main;f;f;g", vec![3]),
                ],
            ),
            (
                "a jump through `t0` to a function start is a tail call, and `jalr ra, X(t0)` a call",
                vec![
                    (0x100, 0x200, "main"),
                    (0x200, 0x300, "c"),
                    (0x300, 0x400, "stub"),
                    (0x400, 0x500, "m"),
                    (0x500, 0x600, "n"),
                ],
                0x100,
                vec![
                    (CALL, 0x200, 0x104, 0x1000),
                    (CALL, 0x300, 0x210, 0xff0),
                    (JR_T0, 0x400, 0, 0xff0),
                    (RET, 0x210, 0, 0xff0),
                    (CALL_T0, 0x500, 0x220, 0xff0),
                    (RET, 0x220, 0, 0xff0),
                    (RET, 0x104, 0, 0x1000),
                ],
                vec![
                    ("main", vec![0, 7]),
                    ("main;c", vec![1, 4, 6]),
                    ("main;c;stub", vec![2]),
                    ("main;c;m", vec![3]),
                    ("main;c;n", vec![5]),
                ],
            ),
            (
                "`jalr ra, X(t0)` calls code outside every function symbol",
                vec![(0x100, 0x200, "main"), (0x200, 0x300, "c")],
                0x100,
                vec![
                    (CALL, 0x200, 0x104, 0x1000),
                    (CALL_T0, 0x400, 0x210, 0xff0),
                    (RET, 0x210, 0, 0xff0),
                    (RET, 0x104, 0, 0x1000),
                ],
                vec![
                    ("main", vec![0, 4]),
                    ("main;c", vec![1, 3]),
                    ("main;c;[unknown]", vec![2]),
                ],
            ),
            (
                "`jalr ra, 0(ra)` to the return address of the running frame returns, as a \
                 NativeAOT runtime helper does",
                vec![(0x100, 0x200, "c"), (0x200, 0x300, "x")],
                0x100,
                vec![
                    (CALL_A2, 0x200, 0x108, 0x1000),
                    (CALL, 0x108, 0x2fc, 0x1000),
                ],
                vec![("c", vec![0, 2]), ("c;x", vec![1])],
            ),
        ] {
            let expected: BTreeMap<String, u64> = expected
                .into_iter()
                .map(|(stack, segments)| {
                    (
                        stack.to_owned(),
                        segments.iter().map(|segment| 1 << segment).sum(),
                    )
                })
                .collect();
            let call_tree = run(functions, entry, &jumps, |index| 1 << index);
            // Each case returns from all its calls, and each frame spends a segment.
            assert!(call_tree.calls.is_empty(), "{case}");
            assert_eq!(call_tree.nodes.len(), expected.len() + 1, "{case}");
            assert_eq!(stacks(call_tree), expected, "{case}");
        }
    }

    #[test]
    fn call_tree_stays_bounded_over_repeated_jumps() {
        // Each frame that runs spends 1.
        for (case, functions, entry, jumps, nodes, expected) in [
            (
                "coroutines without function symbols swap 100 times, each to the continuation of the \
                 last swap of the other and each on a stack of its own",
                Vec::new(),
                0x1000,
                (0..50)
                    .flat_map(|swap| {
                        let (a, b) = (0x1000 + 0x10 * swap, 0x2000 + 0x10 * swap);
                        let resume_b = if swap == 0 { 0x2000 } else { b - 0xc };
                        [
                            (CALL_T0, resume_b, a + 4, 0xb000),
                            (SWAP_RA, a + 4, b + 4, 0xa000),
                        ]
                    })
                    .collect::<Vec<_>>(),
                3,
                vec![("[unknown]", 51), ("[unknown];[unknown]", 50)],
            ),
            (
                "`f` ends with `jal h`, whose continuation has a symbol `g` of its own, and `g` \
                 jumps back to repeat the call 100 times",
                vec![
                    (0x100, 0x108, "f"),
                    (0x108, 0x200, "g"),
                    (0x200, 0x300, "h"),
                ],
                0x104,
                (0..100)
                    .flat_map(|_| {
                        [
                            (CALL, 0x200, 0x108, 0x1000),
                            (RET, 0x108, 0, 0x1000),
                            (JUMP, 0x104, 0, 0x1000),
                        ]
                    })
                    .collect(),
                3,
                vec![("f", 201), ("f;h", 100)],
            ),
            (
                "code without function symbols calls on another stack with `jalr ra, 0(t0)`, and \
                 the callee switches back and returns, 100 times",
                Vec::new(),
                0x1000,
                (0..100)
                    .flat_map(|cycle| {
                        let return_address = 0x1004 + 0x10 * cycle;
                        [
                            (CALL_T0, 0x8000, return_address, 0xb000),
                            (RET, return_address, 0, 0xa000),
                        ]
                    })
                    .collect(),
                3,
                vec![("[unknown]", 101), ("[unknown];[unknown]", 100)],
            ),
        ] {
            let call_tree = run(functions, entry, &jumps, |_| 1);
            assert_eq!(call_tree.nodes.len(), nodes, "{case}");
            assert!(call_tree.calls.is_empty(), "{case}");
            let expected: BTreeMap<String, u64> = expected
                .into_iter()
                .map(|(stack, cost)| (stack.to_owned(), cost))
                .collect();
            assert_eq!(stacks(call_tree), expected, "{case}");
        }
    }

    #[test]
    fn call_tree_cuts_deep_stacks_and_merges_the_cut_ones() {
        // Each of the `MAX_STACK_DEPTH + 3` frames of a recursion spends 1.
        let depth = MAX_STACK_DEPTH + 3;
        let jumps = vec![(CALL, 0x100, 0x104, 0x1000); depth - 1];
        let call_tree = run(vec![(0x100, 0x200, "f")], 0x100, &jumps, |_| 1);
        let profile = call_tree.into_profile("cells", &PeakMemory::new(0..0, &[]));

        // The 4 deepest frames share the stack of `MAX_STACK_DEPTH` frames.
        assert_eq!(profile.pprof.sample.len(), MAX_STACK_DEPTH);
        let deepest = profile.pprof.sample.last().unwrap();
        assert_eq!(deepest.location_id.len(), MAX_STACK_DEPTH);
        assert_eq!(deepest.value, [4, 0, 4, 4]);
        assert_eq!(profile.cost_estimation().cost["a"], depth as u64);
    }

    #[test]
    fn functions_name_each_range_once() {
        // `f` encloses two symbols, `g` has a local alias whose name sorts first, and a second `f`
        // starts at 0x300.
        let symbol_map = SymbolMap::new(vec![
            (0x100, 0x140, true, "f_fast"),
            (0x100, 0x200, true, "f"),
            (0x180, 0x1c0, true, "f_inner"),
            (0x200, 0x300, false, "a_local"),
            (0x200, 0x300, true, "g"),
            (0x300, 0x400, true, "f"),
        ]);
        let lookup = |pc| {
            let (name, range) = symbol_map.lookup(pc);
            (name, symbol_map.names[name].as_str(), range)
        };
        assert_eq!(lookup(0x150), (1, "f", 0x100..0x200));
        assert_eq!(lookup(0x190), (1, "f", 0x100..0x200));
        assert_eq!(lookup(0x250), (2, "g", 0x200..0x300));
        assert_eq!(lookup(0x350), (1, "f", 0x300..0x400));
        assert_eq!(lookup(0x400), (0, UNKNOWN_FRAME, 0x400..u64::MAX));
    }

    #[test]
    fn peak_memory_counts_the_heap_doublewords_outside_the_non_heap_ranges_and_the_stack() {
        // The code and data segments cover 0x1004..0x2000, the executor uses 0x8000..0x9000, and
        // the stack runs from 0xf_ff00 to 0x10_0000.
        for (case, accesses, heap_doublewords) in [
            ("no access", vec![], 0),
            (
                "accesses in the non-heap ranges and the stack only",
                vec![(0x1008, 8), (0x8ff8, 8), (0xf_ff80, 8)],
                0,
            ),
            (
                "an access to the start of a segment that does not start a doubleword",
                vec![(0x1004, 4)],
                0,
            ),
            (
                "heap doublewords around the memory that the executor uses",
                vec![(0x7ff8, 8), (0x8000, 8), (0x9000, 8)],
                2,
            ),
            (
                "heap doublewords beside a loadable doubleword",
                vec![(0x2000, 8), (0x3ff8, 8), (0x1ff8, 8)],
                2,
            ),
            (
                "a heap doubleword above the stack",
                vec![(0x2000, 8), (0x10_0000, 8)],
                2,
            ),
            (
                "an unaligned access touches each doubleword that it covers",
                vec![(0x3004, 8)],
                2,
            ),
            (
                "a doubleword that two accesses touch counts once",
                vec![(0x3000, 8), (0x3004, 4)],
                1,
            ),
            (
                "an access across a page boundary, and accesses that the memory clips",
                vec![(0x1_0ffc, 8), (0x0ff8, 16), (0x20_1000, 8)],
                2,
            ),
        ] {
            let mut memory = PeakMemory::new(
                0x1000..0x20_1000,
                &[0x1004..0x1800, 0x1800..0x2000, 0x8000..0x9000],
            );
            for (sp, write) in [
                (0x10_0000, Load),
                (0x10_0000, Other),
                (0xf_ff00, Other),
                (0xf_ff00, Other),
            ] {
                memory.stack_pointer(sp, write);
            }
            for (address, len) in accesses {
                memory.access(address, len, Frame(1));
            }
            assert_eq!(memory.peak_stack_bytes(), 0x100, "{case}");
            assert_eq!(memory.peak_heap_bytes(), 8 * heap_doublewords, "{case}");
        }
    }

    #[test]
    fn peak_memory_counts_the_stack_span_that_the_program_runs_with() {
        for (case, writes, peak) in [
            (
                "`0x9000` and `0x8f08` compute the address that the top `0x2000` loads from, as \
                 the SP1 entry code does, and the next load replaces `0x5000`, so none of them \
                 counts",
                vec![
                    (0, Other),
                    (0x9000, Other),
                    (0x8f08, Other),
                    (0x2000, Load),
                    (0x2000, Other),
                    (0x1c00, Other),
                    (0x1c00, Other),
                    (0x5000, Other),
                    (0x1800, Load),
                    (0x1800, Other),
                ],
                0x800,
            ),
            (
                "a top that the program reserves a frame on at once counts",
                vec![(0x4000, Other), (0x3c00, Other), (0x3c00, Other)],
                0x400,
            ),
            (
                "the program reserves a 4 KiB frame with one instruction right after it sets the top",
                vec![(0x8000, Other), (0x7000, Other), (0x7000, Other)],
                0x1000,
            ),
            (
                "the deepest value runs only the load that restores the stack pointer from its frame",
                vec![
                    (0x8000, Other),
                    (0x8000, Other),
                    (0x7ff0, Other),
                    (0x8000, Load),
                ],
                0x10,
            ),
            (
                "`auipc sp` and `addi sp, sp, -0x2f4` compute the top `0xa0400000`, as the zesu \
                 entry code does, and the program then reserves a 0x100-byte frame",
                vec![
                    (0xa040_02f4, UpperImmediate),
                    (0xa040_0000, Other),
                    (0xa040_0000, Other),
                    (0xa03f_ff00, Other),
                    (0xa03f_ff00, Other),
                ],
                0x100,
            ),
            (
                "`auipc sp` and `addi sp, sp, -0x800` lower the upper immediate by the most that one \
                 `addi` can, so the run starts at the lower value",
                vec![
                    (0x8800, UpperImmediate),
                    (0x8000, Other),
                    (0x8000, Other),
                    (0x7f00, Other),
                    (0x7f00, Other),
                ],
                0x100,
            ),
            (
                "a whole top that `lui` writes counts, because a step keeps it",
                vec![
                    (0x8000, UpperImmediate),
                    (0x8000, Other),
                    (0x7f00, Other),
                    (0x7f00, Other),
                ],
                0x100,
            ),
            (
                "`lui sp, 0x80000` and `addiw sp, sp, -16` compute the top `0x7ffffff0`, which is \
                 within one `addiw` of the sign-extended `lui` value only in the low 32 bits",
                vec![
                    (0xffff_ffff_8000_0000, UpperImmediate),
                    (0x7fff_fff0, Other),
                    (0x7fff_fff0, Other),
                    (0x7fff_fef0, Other),
                    (0x7fff_fef0, Other),
                ],
                0x100,
            ),
            (
                "a whole top that `lui` writes counts when the next step lowers it by more than one \
                 `addi` can, as `sub sp, sp, t0` reserves a 4 KiB frame",
                vec![(0x8000, UpperImmediate), (0x7000, Other), (0x7000, Other)],
                0x1000,
            ),
            (
                "an upper immediate that writes the kept value again leaves the run as it is",
                vec![
                    (0x9000, Other),
                    (0x9000, Other),
                    (0x9000, UpperImmediate),
                    (0x8ff0, Other),
                    (0x8ff0, Other),
                ],
                0x10,
            ),
            (
                "a `lui` starts a new run, so the partial value of a switch to another stack does \
                 not count",
                vec![
                    (0x8000, Other),
                    (0x8000, Other),
                    (0x7f00, Other),
                    (0x7f00, Other),
                    (0x1000, UpperImmediate),
                    (0x9000, Other),
                    (0x9000, Other),
                ],
                0x1100,
            ),
        ] {
            let mut memory = PeakMemory::new(0..0, &[]);
            for (sp, write) in writes {
                memory.stack_pointer(sp, write);
            }
            assert_eq!(memory.peak_stack_bytes(), peak, "{case}");
        }
    }
}

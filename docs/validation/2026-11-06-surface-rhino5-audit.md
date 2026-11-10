# Surface menu — Rhino 5 audit (add-on `addons/surfaces`)

Date 2026-11-06, branch `codex/surfaces`. Matrix 9 runs Rhino 5 commands, so each Surface
command is compared with its Rhino 5 help page (prompt order, command-line options, dialog
options). Sources: Rhino 5 help, Surface Sidebar and Surface Tools toolbar maps
(`docs.mcneel.com/rhino/5/help/en-us/...`), Rhino 5 User's Guide (`ref/rhino5`). Rhino 5
help pages mostly do not state defaults; defaults below marked "OM9" are OM9 decisions.

Legend: **Flow** = prompt order like Rhino 5; **Opt** = options present; **Geo** = geometry
exact / within 0.001 mm; **Perf** = measured.

## Built-in vs add-on (measured, release)

| Test | Built-in OM9 | Add-on before | Add-on now |
|---|---|---|---|
| Loft two r5 circles → cylinder | 0.0042 mm, 0.07 ms | 0.0002 mm, 0.23 ms | < 1e-9 mm |
| Sweep1 r1 circle around R10 ring → torus | 0.0011 mm, 2.1 ms | 0.0002 mm, 0.61 ms | < 1e-9 mm (exact revolution) |
| Grid fit 400 × 400 (one direction) | – | 45 ms (dense) | 10 ms (banded) |

## Commands

| Command (Rhino 5) | Rhino 5 flow / options | Add-on today | Gaps |
|---|---|---|---|
| Loft | curves in order (Point at ends); seams: Flip, Automatic, Natural; dialog Style Loose/Normal/Tight/Straight/Uniform, Closed, Match start/end tangent, SplitAtTangents, Do not simplify / Rebuild N / Refit | **done**: select in order, Point at ends, seams Natural/Automatic (setting), Style (5), Closed, Do not simplify / Rebuild N / Refit tol; exact through sections | Match start/end tangent; interactive seam Flip; SplitAtTangents |
| Sweep1 | rail, then cross sections (Point), seams; Style Freeform/Roadlike Top/Right/Front, Align with surface, Closed, Global shape blending, Untrimmed miters, Simplify, Refit rail | **done**: Select rail → cross sections (Point) → Enter → options with preview (Style Freeform/Roadlike Top/Right/Front, Closed, GlobalBlend, Simplify None/Rebuild/Refit) → Enter; RMF frames; exact sections, stations to 0.001 mm; straight rail exact extrusion, circular rail exact revolution | Align with surface; Untrimmed miters; interactive seams |
| Sweep2 | two rails, cross sections (Point), seams; Maintain height, Closed, Preserve first/last, Refit rails, edge continuity, Add slash, simplify | **done**: first rail → second rail → cross sections (Point) → Enter → options (MaintainHeight, Closed, Simplify) → Enter; edges on the rails, stations to 0.001 mm | Preserve first/last and edge continuity (need surface edges); Refit rails; Add slash |
| Revolve | curves, axis start, axis end, (start angle), revolution angle; DeleteInput, Deformable, PointCount, FullCircle, AskForStartAngle, SplitAtTangents | **done**: curves → axis start → axis end → Revolution angle <360> (typed, Enter, or click around the axis; preview follows the pointer); StartAngle, FullCircle, Deformable + PointCount, DeleteInput; exact rational otherwise | SplitAtTangents (polycurve input) |
| RailRevolve | profile, rail, axis start, axis end; ScaleHeight | **done**: Select profile curve → Select rail curve → axis start → axis end; ScaleHeight; exact tensor product (no sampling) | SplitAtTangents |
| EdgeSrf | select 2–4 curves/edges, Enter | **done**: exact Coons on poles (2, 3, 4 curves) | surface edges as input (host) |
| PlanarSrf | planar curves; inner curves = holes; overlapping → separate | **done**: exact trimmed face (`Geo::Trimmed` → OCCT face, any winding, several holes; kernel tests area 100−4π, 9π, 100−3.25π); containment by whole curve, overlapping curves → separate surfaces | – |
| Plane | rectangle (corner, corner / length); 3Point, Vertical (toolbar variants); Deformable, U/V degree, point count | **done**: Rectangle prompts (corner/corner, typed length/width, Center, 3Point, Vertical), Deformable + degree + point count; Rounded refused | – |
| NetworkSrf | curves both directions (NoAutoSort); tolerances edge/interior/angle; edge matching Loose/Position/Tangency/Curvature | **done**: auto sort by crossings, Gordon surface through every curve on exact curves (edge/interior tolerance, crossing gap); 3×4 network ≈ 3 ms | NoAutoSort; closed curve networks; Tangency/Curvature edge matching (surface edges) |
| Patch | points/curves/edges; sample spacing, spans U/V, stiffness, adjust tangency, auto trim, starting surface | **done**: fit through curves and points (Sample point spacing, U/V spans, Stiffness), Automatic trim to the closed outer curve (trimmed face, curves drawn on the surface; kernel test on a curved face) | Adjust tangency; Starting surface / pull / Preserve edges |
| ExtrudeCrv (Extrude All) | curve, distance by two points; Direction, BothSides, Solid, ToBoundary, DeleteInput, SetBasePoint | **done**: select → Extrusion distance (typed, Enter, click), BothSides, DeleteInput; exact | Direction, Solid, ToBoundary, SetBasePoint (need B-Rep solids, round 3) |
| FilletSrf | first surface (side), second surface (side); Radius, Extend, Trim, Split | radius + flips in settings | side picking; Extend/Trim/Split |
| ChamferSrf | as FilletSrf; Distances (two), Extend, Trim, Split | one distance | two distances; Extend/Trim/Split |
| BlendSrf | edge, adjacent, edges to blend to, adjacent, adjust; per-end continuity Position…G4, bulge sliders, Lock, AddShapes, Planar sections, Same height | bulge settings | continuity per end; shapes; edge chaining |
| OffsetSrf | surface, distance (+/− by arrow, click to flip); Corner Sharp/Round, FlipAll, Solid, Loose, Tolerance (0), BothSides, DeleteInput | distance + flip in settings | distance prompt; Corner; Solid; Loose; BothSides |
| MatchSrf | edge to change, target edge/curve; Continuity, Preserve other end, Average, closest points, Refine, isocurve direction, MultipleMatches | continuity setting | the other options |
| ExtendSrf | edge, distance; Type Smooth/Line | length then edge | prompt order; Type |
| Fin (Extrude normal) | curve on surface, surface, distance | height setting | check help |
| TweenSurfaces, Variable Fillet/Blend/Chamfer/Offset, MergeSrf, Drape, Heightfield, SplitIsocurve, ShrinkTrimmedSrf, Untrim, Smash, UnrollSrf, SoftEditSrf, SrfSeam, SetSurfaceTangent, FitSrf, Rebuild, CutPlane | read the Rhino 5 page when each is worked | implemented, settings-driven | flow and options per page |

## Rounds

1. **Done 2026-11-07.** Exact NURBS base; Loft, Revolve, EdgeSrf exact; banded grid fit; Plane,
   PlanarSrf (exact trimmed faces through OCCT), Extrude flows and options.
2. **Done 2026-11-10.** Sweep1, Sweep2, Profile/Multi sweep, RailRevolve, NetworkSrf, Patch —
   Rhino prompts, options, exact sections.
3. Fillet, Chamfer, Blend, Offset, Match, Merge, Extend and the edit tools — prompts and
   options per page.

Each round: speed of preview and result on empty and 5,000/20,000-object files, accuracy
against exact references, Vietnamese strings, package tests.

## Speed (round 2, release, Linux container, `addon_bench`)

Whole command, from start to its one commit (median of 20); built-in Line measured in the
same run for comparison:

| Document | Loft 2 curves | Sweep1 | Revolve 360 | Built-in Line |
|---|---|---|---|---|
| 2 objects | 2.9 ms | 10.3 ms | 3.2 ms | 0.12 ms |
| 5,000 lines | 65.4 ms | 67.1 ms | 64.7 ms | 62.1 ms |
| 20,000 lines | 324.5 ms | 323.8 ms | 333.6 ms | 299.4 ms |

In large documents the time is OM9's commit of one new object (the same incremental path as
its own commands: at 20,000 objects ≈ 230–320 ms commit, the add-on step ≈ 5 ms). Pointer move
while Revolve's preview follows the pointer: 0.40–0.81 ms per move, independent of the
document size. Geometry alone (release): Sweep1 ring with two sections 17 ms (55 stations,
0.0008 mm), NetworkSrf 3×4 3.3 ms, Patch 10×10 spans 0.9 ms, RailRevolve exact < 0.1 ms.

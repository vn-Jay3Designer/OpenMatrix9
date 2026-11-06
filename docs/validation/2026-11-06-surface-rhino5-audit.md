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
| Sweep1 r1 circle around R10 ring → torus | 0.0011 mm, 2.1 ms | 0.0002 mm, 0.61 ms | (round 2) |
| Grid fit 400 × 400 (one direction) | – | 45 ms (dense) | 10 ms (banded) |

## Commands

| Command (Rhino 5) | Rhino 5 flow / options | Add-on today | Gaps |
|---|---|---|---|
| Loft | curves in order (Point at ends); seams: Flip, Automatic, Natural; dialog Style Loose/Normal/Tight/Straight/Uniform, Closed, Match start/end tangent, SplitAtTangents, Do not simplify / Rebuild N / Refit | select all, Enter; Style (5), Closed; **exact sections** | Point; seam Flip/Automatic/Natural; Simplify; Match tangents |
| Sweep1 | rail, then cross sections (Point), seams; Style Freeform/Roadlike Top/Right/Front, Align with surface, Closed, Global shape blending, Untrimmed miters, Simplify, Refit rail | settings `sw_stations`; sampled (0.0002 mm) | ordered prompts; Style; Closed; Point; simplify; exact |
| Sweep2 | two rails, cross sections (Point), seams; Maintain height, Closed, Preserve first/last, Refit rails, edge continuity, Add slash, simplify | select all, Enter; Maintain height | ordered prompts; Closed; Point; Preserve; continuity |
| Revolve | curves, axis start, axis end, (start angle), revolution angle; DeleteInput, Deformable, PointCount, FullCircle, AskForStartAngle, SplitAtTangents | **done**: curves → axis start → axis end → Revolution angle <360> (typed, Enter, or click around the axis; preview follows the pointer); StartAngle, FullCircle, Deformable + PointCount, DeleteInput; exact rational otherwise | SplitAtTangents (polycurve input) |
| RailRevolve | profile, rail, axis start, axis end; ScaleHeight | profile+rail then axis; sampled | ScaleHeight; exact |
| EdgeSrf | select 2–4 curves/edges, Enter | **exact Coons on poles** | surface edges as input (host) |
| PlanarSrf | planar curves; inner curves = holes; overlapping → separate | holes supported | overlapping curves → separate surfaces |
| Plane | rectangle (corner, corner / length); 3Point, Vertical (toolbar variants); Deformable, U/V degree, point count | two corners | 3Point, Vertical, Center; typed length/width; Deformable |
| NetworkSrf | curves both directions (NoAutoSort); tolerances edge/interior/angle; edge matching Loose/Position/Tangency/Curvature | families by direction; sampled | tolerances; edge matching; exact edges |
| Patch | points/curves/edges; sample spacing, spans U/V, stiffness, adjust tangency, auto trim, starting surface | Coons from boundary | real patch fit with options |
| ExtrudeCrv (Extrude All) | curve, distance by two points; Direction, BothSides, Solid, ToBoundary, DeleteInput, SetBasePoint | distance + both sides in settings | Direction/Solid/ToBoundary; typed distance prompt |
| FilletSrf | first surface (side), second surface (side); Radius, Extend, Trim, Split | radius + flips in settings | side picking; Extend/Trim/Split |
| ChamferSrf | as FilletSrf; Distances (two), Extend, Trim, Split | one distance | two distances; Extend/Trim/Split |
| BlendSrf | edge, adjacent, edges to blend to, adjacent, adjust; per-end continuity Position…G4, bulge sliders, Lock, AddShapes, Planar sections, Same height | bulge settings | continuity per end; shapes; edge chaining |
| OffsetSrf | surface, distance (+/− by arrow, click to flip); Corner Sharp/Round, FlipAll, Solid, Loose, Tolerance (0), BothSides, DeleteInput | distance + flip in settings | distance prompt; Corner; Solid; Loose; BothSides |
| MatchSrf | edge to change, target edge/curve; Continuity, Preserve other end, Average, closest points, Refine, isocurve direction, MultipleMatches | continuity setting | the other options |
| ExtendSrf | edge, distance; Type Smooth/Line | length then edge | prompt order; Type |
| Fin (Extrude normal) | curve on surface, surface, distance | height setting | check help |
| TweenSurfaces, Variable Fillet/Blend/Chamfer/Offset, MergeSrf, Drape, Heightfield, SplitIsocurve, ShrinkTrimmedSrf, Untrim, Smash, UnrollSrf, SoftEditSrf, SrfSeam, SetSurfaceTangent, FitSrf, Rebuild, CutPlane | read the Rhino 5 page when each is worked | implemented, settings-driven | flow and options per page |

## Rounds

1. Exact NURBS base; Loft, Revolve, EdgeSrf exact (done); banded grid fit (done); Plane,
   PlanarSrf, Extrude flows and options.
2. Sweep1, Sweep2, Profile/Multi sweep, RailRevolve, NetworkSrf, Patch — Rhino prompts,
   options, exact sections.
3. Fillet, Chamfer, Blend, Offset, Match, Merge, Extend and the edit tools — prompts and
   options per page.

Each round: speed of preview and result on empty and 5,000/20,000-object files, accuracy
against exact references, Vietnamese strings, package tests.

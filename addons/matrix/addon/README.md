# OM9 Matrix

Gói add-on gồm các lệnh kiểu Matrix 9 cho OM9. Sau khi cài và bật gói, lệnh
gọi được từ icon trên menu, từ dòng lệnh (tên lệnh hoặc tên gõ tắt) và từ F6. Khi gói
đang bật, lệnh của add-on được dùng thay cho lệnh cùng tên có sẵn trong OM9; tắt gói
để quay lại lệnh có sẵn.

Mỗi lệnh có một trang trong `commands/<Menu>/`.

Tổng cộng: 114 lệnh.

## Curve (13 lệnh)

| Mã lệnh | Lệnh | Vị trí menu |
|---|---|---|
| `CurvePolylinePolyline` | [Polyline](commands/Curve/CurvePolylinePolyline.md) | Curve #1 |
| `CurveLineSingleLine` | [Single Line](commands/Curve/CurveLineSingleLine.md) | Curve #2 |
| `CurveFreeFormInterpolatePoints` | [Interpolate Points](commands/Curve/CurveFreeFormInterpolatePoints.md) | Curve #3 |
| `CurveFreeFormControlPoints` | [Control Point Curve](commands/Curve/CurveFreeFormControlPoints.md) | Curve #41 |
| `CurveRectangleCornertoCorner` | [Rectangle](commands/Curve/CurveRectangleCornertoCorner.md) | Curve #4 |
| `CurveCircleCenterRadius` | [Circle](commands/Curve/CurveCircleCenterRadius.md) | Curve #5 |
| `CurveEllipseFromCenter` | [Ellipse](commands/Curve/CurveEllipseFromCenter.md) | Curve #6 |
| `CurveArcCenterStartAngle` | [Arc](commands/Curve/CurveArcCenterStartAngle.md) | Curve #7 |
| `CurveArcStartEndDirection` | [Arc Start End Direction](commands/Curve/CurveArcStartEndDirection.md) | Curve #8 |
| `CurvePolygonCenterRadius` | [Polygon](commands/Curve/CurvePolygonCenterRadius.md) | Curve #28 |
| `CurveSpiral` | [Spiral](commands/Curve/CurveSpiral.md) | Curve #29 |
| `CurveHelix` | [Helix](commands/Curve/CurveHelix.md) | Curve #30 |
| `CurvePointObjectSinglePoint` | [Point](commands/Curve/CurvePointObjectSinglePoint.md) | Curve #31 |

## Cutters (10 lệnh)

| Mã lệnh | Lệnh | Vị trí menu |
|---|---|---|
| `BuilderGemCutter` | [Gem Cutter](commands/Cutters/BuilderGemCutter.md) | Cutters #1 |
| `BuilderGemCutterLibrary` | [Gem Cutter Library](commands/Cutters/BuilderGemCutterLibrary.md) | Cutters #2 |
| `BuilderBezelCutter` | [Bezel Cutter](commands/Cutters/BuilderBezelCutter.md) | Settings #5 |
| `BuilderChannelBuilder` | [Channel Builder](commands/Cutters/BuilderChannelBuilder.md) | Cutters #3 |
| `BuilderBrightCutChannel` | [Bright Cut Channel](commands/Cutters/BuilderBrightCutChannel.md) | Cutters #5 |
| `BuilderMicroProngCutter` | [Micro Prong Cutter](commands/Cutters/BuilderMicroProngCutter.md) | Cutters #4 |
| `BuilderBrightCutter` | [Bright Cutter](commands/Cutters/BuilderBrightCutter.md) | Cutters #6 |
| `BuilderPlaneandCubeCutters` | [Plane and Cube Cutters](commands/Cutters/BuilderPlaneandCubeCutters.md) | Cutters #9 |
| `OthersCuttoFingerRail` | [Cut to Finger Rail](commands/Cutters/OthersCuttoFingerRail.md) | Cutters #8 |
| `BuilderQuadFlip` | [Quad Flip](commands/Cutters/BuilderQuadFlip.md) | Cutters #10 |

## Gems (12 lệnh)

| Mã lệnh | Lệnh | Vị trí menu |
|---|---|---|
| `BuilderGemLoader` | [Gem Loader](commands/Gems/BuilderGemLoader.md) | Gems #1 |
| `BuilderEmerald` | [Emerald Builder](commands/Gems/BuilderEmerald.md) | Gems #13 |
| `BuilderBaguette` | [Baguette Builder](commands/Gems/BuilderBaguette.md) | Gems #14 |
| `BuilderCustomGem` | [Custom Gem](commands/Gems/BuilderCustomGem.md) | Gems #12 |
| `BuilderHalo` | [Halo Builder](commands/Gems/BuilderHalo.md) | Gems #10 |
| `BuilderGemReporter` | [Gem Reporter](commands/Gems/BuilderGemReporter.md) | Gems #19 |
| `BuilderGemMap` | [Gem Map](commands/Gems/BuilderGemMap.md) | Gems #20 |
| `BuilderGemGuides` | [Gem Guides](commands/Gems/BuilderGemGuides.md) | Gems #8 |
| `BuilderGemBetweenTwoCurves` | [Gems Between Two Curves](commands/Gems/BuilderGemBetweenTwoCurves.md) | Gems #17 |
| `BuilderGemUpdate` | [Gem Update](commands/Gems/BuilderGemUpdate.md) | Gems #34 |
| `BuilderGemOnCrv` | [Gem On Crv](commands/Gems/BuilderGemOnCrv.md) | Gems #2 |
| `BuilderGemCountOnCrv` | [Gem Count On Crv](commands/Gems/BuilderGemCountOnCrv.md) | Gems #4 |

## Settings (5 lệnh)

| Mã lệnh | Lệnh | Vị trí menu |
|---|---|---|
| `BuilderProngAdder` | [Prong Builder](commands/Settings/BuilderProngAdder.md) | Settings #6 |
| `BuilderBezel` | [Bezel Builder](commands/Settings/BuilderBezel.md) | Settings #2 |
| `BuilderHead` | [Head Builder](commands/Settings/BuilderHead.md) | Settings #1 |
| `BuilderBeadOnCrv` | [Bead on Curve](commands/Settings/BuilderBeadOnCrv.md) | Settings #11 |
| `BuilderChannelBorderCurve` | [Channel Border Curve](commands/Settings/BuilderChannelBorderCurve.md) | Settings #12 |

## Solid (30 lệnh)

| Mã lệnh | Lệnh | Vị trí menu |
|---|---|---|
| `SolidUnion` | [Boolean Union](commands/Solid/SolidUnion.md) | Solid #1 |
| `SolidDifference` | [Boolean Difference](commands/Solid/SolidDifference.md) | Solid #2 |
| `SolidIntersection` | [Boolean Intersection](commands/Solid/SolidIntersection.md) | Solid #3 |
| `SolidBooleanTwoObjects` | [Boolean Two Objects](commands/Solid/SolidBooleanTwoObjects.md) | Solid #4 |
| `SolidCapPlanarHoles` | [Cap Planar Holes](commands/Solid/SolidCapPlanarHoles.md) | Solid #5 |
| `SolidExtractSurface` | [Extract Surface](commands/Solid/SolidExtractSurface.md) | Solid #6 |
| `SolidFilletEdgeFilletEdge` | [Fillet Edge](commands/Solid/SolidFilletEdgeFilletEdge.md) | Solid #7 |
| `SolidText` | [Text](commands/Solid/SolidText.md) | Solid #8 |
| `SolidExtrudePlanarCurveStraight` | [Extrude Planar Curve](commands/Solid/SolidExtrudePlanarCurveStraight.md) | Solid #9 |
| `SolidGVDAllExtrude` | [Extrude All](commands/Solid/SolidGVDAllExtrude.md) | Solid #10 |
| `SolidPipe` | [Pipe](commands/Solid/SolidPipe.md) | Solid #11 |
| `SolidBoxCornertoCornerHeight` | [Box](commands/Solid/SolidBoxCornertoCornerHeight.md) | Solid #12 |
| `SolidSphereCenterRadius` | [Sphere](commands/Solid/SolidSphereCenterRadius.md) | Solid #13 |
| `SolidEllipsoidFromCenter` | [Ellipsoid](commands/Solid/SolidEllipsoidFromCenter.md) | Solid #14 |
| `SolidTorus` | [Torus](commands/Solid/SolidTorus.md) | Solid #15 |
| `SolidCylinder` | [Cylinder](commands/Solid/SolidCylinder.md) | Solid #16 |
| `SolidTube` | [Tube](commands/Solid/SolidTube.md) | Solid #17 |
| `SolidPyramid` | [Pyramid](commands/Solid/SolidPyramid.md) | Solid #18 |
| `SolidCone` | [Cone](commands/Solid/SolidCone.md) | Solid #19 |
| `SolidTruncatedCone` | [Truncated Cone](commands/Solid/SolidTruncatedCone.md) | Solid #20 |
| `SolidBoss` | [Boss](commands/Solid/SolidBoss.md) | Solid #21 |
| `SolidRib` | [Rib](commands/Solid/SolidRib.md) | Solid #22 |
| `SolidSlab` | [Slab](commands/Solid/SolidSlab.md) | Solid #23 |
| `SolidSolidEditToolsHolesMakeHole` | [Make Hole](commands/Solid/SolidSolidEditToolsHolesMakeHole.md) | Solid #24 |
| `SolidSolidEditToolsHolesArrayHole` | [Array Hole](commands/Solid/SolidSolidEditToolsHolesArrayHole.md) | Solid #25 |
| `SolidSolidEditToolsHolesArrayHolePolar` | [Array Hole Polar](commands/Solid/SolidSolidEditToolsHolesArrayHolePolar.md) | Solid #26 |
| `SolidSolidEditToolsHolesMoveHole` | [Move Hole](commands/Solid/SolidSolidEditToolsHolesMoveHole.md) | Solid #27 |
| `SolidSolidEditToolsFacesMoveFace` | [Move Face](commands/Solid/SolidSolidEditToolsFacesMoveFace.md) | Solid #28 |
| `SolidShell` | [Shell](commands/Solid/SolidShell.md) | Solid #29 |
| `SolidPtOn` | [Solid Points On](commands/Solid/SolidPtOn.md) | Solid #30 |

## Transform (44 lệnh)

| Mã lệnh | Lệnh | Vị trí menu |
|---|---|---|
| `TopIconMove` | [Move](commands/Transform/TopIconMove.md) | Top 11 #5 |
| `TopIconDuplicate` | [Duplicate](commands/Transform/TopIconDuplicate.md) | Top 11 #1 |
| `TopIconRotate` | [Rotate](commands/Transform/TopIconRotate.md) | Top 11 #6 |
| `TopIconMirror` | [Mirror](commands/Transform/TopIconMirror.md) | Top 11 #4 |
| `TransformRotate3D` | [Rotate 3D](commands/Transform/TransformRotate3D.md) | Transform #7 |
| `TransformScaleScale3D` | [Scale 3D](commands/Transform/TransformScaleScale3D.md) | Transform #1 |
| `TransformScaleScale2D` | [Scale 2D](commands/Transform/TransformScaleScale2D.md) | Transform #2 |
| `TransformScaleScale1D` | [Scale 1D](commands/Transform/TransformScaleScale1D.md) | Transform #3 |
| `TransformArrayPolar` | [Array Polar](commands/Transform/TransformArrayPolar.md) | Transform #9 |
| `TransformArrayArrayLinear` | [Array Linear](commands/Transform/TransformArrayArrayLinear.md) | Transform #22 |
| `TransformArrayRectangular` | [Array](commands/Transform/TransformArrayRectangular.md) | Transform #25 |
| `TransformScaleNonUniformScale` | [Scale Non-Uniform](commands/Transform/TransformScaleNonUniformScale.md) | Transform #4 |
| `TransformScaleScaleByPlane` | [Scale by Plane](commands/Transform/TransformScaleScaleByPlane.md) | Transform #30 |
| `TransformOrient2Points` | [Orient 2 Points](commands/Transform/TransformOrient2Points.md) | Transform #5 |
| `TransformOrient3Points` | [Orient 3 Points](commands/Transform/TransformOrient3Points.md) | Transform #6 |
| `TransformShear` | [Shear](commands/Transform/TransformShear.md) | Transform #33 |
| `TransformSymmetry` | [Symmetry](commands/Transform/TransformSymmetry.md) | Transform #18 |
| `TransformOrientRemaptoCPlane` | [Remap to CPlane](commands/Transform/TransformOrientRemaptoCPlane.md) | Transform #37 |
| `TransformMoveUVN` | [Move UVN](commands/Transform/TransformMoveUVN.md) | Transform #38 |
| `BuilderMakeDownFacing` | [Make Down Facing](commands/Transform/BuilderMakeDownFacing.md) | Transform #36 |
| `TransformBend` | [Bend](commands/Transform/TransformBend.md) | Transform #8 |
| `TransformTaper` | [Taper](commands/Transform/TransformTaper.md) | Transform #32 |
| `TransformTwist` | [Twist](commands/Transform/TransformTwist.md) | Transform #34 |
| `TransformStretch` | [Stretch](commands/Transform/TransformStretch.md) | Transform #31 |
| `TransformMaelstrom` | [Maelstrom](commands/Transform/TransformMaelstrom.md) | Transform #35 |
| `TransformSoftMove` | [Soft Move](commands/Transform/TransformSoftMove.md) | Transform #39 |
| `TransformSetPoints` | [Set Points](commands/Transform/TransformSetPoints.md) | Transform #29 |
| `TransformProjecttoCPlane` | [Project To C-Plane](commands/Transform/TransformProjecttoCPlane.md) | Transform #10 |
| `TransformFlowAlongCurve` | [Flow Along Curve](commands/Transform/TransformFlowAlongCurve.md) | Transform #26 |
| `TransformFlowAlongSurface` | [Flow Along Surface](commands/Transform/TransformFlowAlongSurface.md) | Transform #27 |
| `TransformSplop` | [Splop](commands/Transform/TransformSplop.md) | Transform #28 |
| `gvSmartFlow` | [Smart Flow](commands/Transform/gvSmartFlow.md) | Transform #11 |
| `gvSmartFlowRigid` | [Smart Flow Rigid](commands/Transform/gvSmartFlowRigid.md) | Transform #12 |
| `gvSmartFlowBreak` | [Smart Flow Break](commands/Transform/gvSmartFlowBreak.md) | Transform #13 |
| `gvSmartFlowPullDownCurves` | [Smart Flow Pull Down Curves](commands/Transform/gvSmartFlowPullDownCurves.md) | Transform #14 |
| `TransformSmartPattern` | [Smart Pattern](commands/Transform/TransformSmartPattern.md) | Transform #15 |
| `TransformCageEditingCreateCage` | [Create Cage](commands/Transform/TransformCageEditingCreateCage.md) | Transform #40 |
| `TransformCageEditingCageEdit` | [Cage Edit](commands/Transform/TransformCageEditingCageEdit.md) | Transform #20 |
| `TransformArrayAlongCurve` | [Array Along Curve](commands/Transform/TransformArrayAlongCurve.md) | Transform #21 |
| `TransformArrayAlongSurface` | [Array Along Surface](commands/Transform/TransformArrayAlongSurface.md) | Transform #23 |
| `TransformArrayAlongCurveonSurface` | [Array Along Curve on Surface](commands/Transform/TransformArrayAlongCurveonSurface.md) | Transform #24 |
| `TransformOrientOnSurface` | [Orient on Surface](commands/Transform/TransformOrientOnSurface.md) | Transform #16 |
| `TransformOrientPerpendiculartoCurve` | [Orient Perpendicular to Curve](commands/Transform/TransformOrientPerpendiculartoCurve.md) | Transform #17 |
| `TransformSolidPtOn` | [Solid Points ON](commands/Transform/TransformSolidPtOn.md) | Transform #19 |

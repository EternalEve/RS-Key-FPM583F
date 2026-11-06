#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Copyright (C) 2026 RS-Key contributors
"""Measure and mesh the official board STEP with an installed OCCT 7.9 kernel."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess


STEP_SHA256 = "1d4a3f221135fa6bfb4e5ab6611f0039b1e9da6ac0485e29b8ff0202f4a0389c"
FOLDER = Path(__file__).resolve().parent / "build" / "reference"
LIBRARIES = (
    "TKDESTEP", "TKXSBase", "TKXCAF", "TKCAF", "TKLCAF", "TKCDF",
    "TKMesh", "TKDESTL", "TKTopAlgo", "TKBRep", "TKGeomBase", "TKG3d",
    "TKG2d", "TKMath", "TKernel",
)

READER = r'''
// SPDX-License-Identifier: AGPL-3.0-only
// Copyright (C) 2026 RS-Key contributors
#include <Bnd_Box.hxx>
#include <BRepBndLib.hxx>
#include <BRepCheck_Analyzer.hxx>
#include <BRepMesh_IncrementalMesh.hxx>
#include <STEPCAFControl_Reader.hxx>
#include <StlAPI_Writer.hxx>
#include <TDataStd_Name.hxx>
#include <TDF_LabelSequence.hxx>
#include <TDocStd_Document.hxx>
#include <TopExp_Explorer.hxx>
#include <XCAFApp_Application.hxx>
#include <XCAFDoc_DocumentTool.hxx>
#include <XCAFDoc_ShapeTool.hxx>
#include <fstream>
#include <iomanip>
#include <iostream>
#include <stdexcept>
#include <string>

static void bounds(std::ostream& out, const TopoDS_Shape& shape) {
    Bnd_Box box;
    BRepBndLib::AddOptimal(shape, box, false, false);
    if (box.IsVoid()) throw std::runtime_error("Empty shape");
    double a, b, c, d, e, f;
    box.Get(a, b, c, d, e, f);
    out << '[' << a << ',' << b << ',' << c << ',' << d << ',' << e << ',' << f << ']';
}

static std::string name(const TDF_Label& label) {
    Handle(TDataStd_Name) attr;
    if (!label.FindAttribute(TDataStd_Name::GetID(), attr)) return "unnamed";
    return TCollection_AsciiString(attr->Get(), '?').ToCString();
}

static void visit(const Handle(XCAFDoc_ShapeTool)& tool, const TDF_Label& label,
                  const TopLoc_Location& parent, const std::string& path,
                  std::ostream& out, bool& first) {
    TDF_Label definition = label;
    TopLoc_Location location = parent;
    if (tool->IsReference(label)) {
        if (!tool->GetReferredShape(label, definition))
            throw std::runtime_error("Unresolved instance");
        location = parent * tool->GetLocation(label);
    }
    const auto shape = tool->GetShape(definition).Moved(location);
    const auto fullPath = path + '/' + name(definition);
    if (!first) out << ",\n";
    first = false;
    out << "{\"path\":" << std::quoted(fullPath) << ",\"bounds\":";
    bounds(out, shape);
    if (name(definition) == "Key1" || name(definition) == "Key2") {
        Bnd_Box whole, top;
        BRepBndLib::AddOptimal(shape, whole, false, false);
        double a, b, c, d, e, zmax;
        whole.Get(a, b, c, d, e, zmax);
        for (TopExp_Explorer faces(shape, TopAbs_FACE); faces.More(); faces.Next()) {
            Bnd_Box face;
            BRepBndLib::AddOptimal(faces.Current(), face, false, false);
            double f;
            face.Get(a, b, c, d, e, f);
            if (c > zmax - 0.0001) top.Add(face);
        }
        if (top.IsVoid()) throw std::runtime_error("No flat actuator top");
        top.Get(a, b, c, d, e, zmax);
        out << ",\"actuator_top_bounds\":[" << a << ',' << b << ',' << c
            << ',' << d << ',' << e << ',' << zmax << ']';
    }
    out << '}';
    TDF_LabelSequence children;
    tool->GetComponents(definition, children);
    // Instance transforms belong to the reference; definition locations apply once.
    const auto childLocation = location * tool->GetLocation(definition);
    for (int i = 1; i <= children.Length(); ++i)
        visit(tool, children.Value(i), childLocation, fullPath, out, first);
}

int main(int argc, char** argv) {
    try {
        if (argc != 4) throw std::runtime_error("Expected STEP, JSON and STL paths");
        Handle(TDocStd_Document) doc;
        XCAFApp_Application::GetApplication()->NewDocument("MDTV-XCAF", doc);
        STEPCAFControl_Reader reader;
        if (reader.ReadFile(argv[1]) != IFSelect_RetDone || !reader.Transfer(doc))
            throw std::runtime_error("STEP import failed");
        auto tool = XCAFDoc_DocumentTool::ShapeTool(doc->Main());
        TDF_LabelSequence roots;
        tool->GetFreeShapes(roots);
        if (roots.Length() != 1) throw std::runtime_error("Expected one board assembly");
        auto shape = tool->GetShape(roots.Value(1));
        BRepMesh_IncrementalMesh mesh(shape, 0.05, false, 0.3, true);
        StlAPI_Writer writer;
        writer.ASCIIMode() = false;
        if (!writer.Write(shape, argv[3])) throw std::runtime_error("STL export failed");
        std::ofstream out(argv[2]);
        out << std::setprecision(12) << "{\"units\":\"mm\",\"bounds\":";
        bounds(out, shape);
        out << ",\"brep_valid\":" << (BRepCheck_Analyzer(shape).IsValid() ? "true" : "false")
            << ",\"mesh_flags\":" << mesh.GetStatusFlags() << ",\"nodes\":[\n";
        bool first = true;
        visit(tool, roots.Value(1), TopLoc_Location(), "", out, first);
        out << "]}\n";
        if (!out) throw std::runtime_error("JSON export failed");
        return 0;
    } catch (const Standard_Failure& error) {
        std::cerr << error.GetMessageString() << '\n';
    } catch (const std::exception& error) {
        std::cerr << error.what() << '\n';
    }
    return 1;
}
'''


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--occt-root", type=Path, required=True)
    parser.add_argument("--step", type=Path, default=FOLDER / "RP2350-Zero-CM.step")
    args = parser.parse_args()
    digest = hashlib.sha256(args.step.read_bytes()).hexdigest()
    if digest != STEP_SHA256:
        parser.error("STEP differs from the verified official board model")
    FOLDER.mkdir(parents=True, exist_ok=True)
    source = FOLDER / "step-reader.cxx"
    executable = FOLDER / "step-reader"
    source.write_text(READER)
    subprocess.run([
        "c++", "-std=c++17", "-O2", "-o", str(executable), str(source),
        f"-I{args.occt_root / 'include' / 'opencascade'}",
        f"-L{args.occt_root / 'lib'}", f"-Wl,-rpath,{args.occt_root / 'lib'}",
        *(f"-l{library}" for library in LIBRARIES),
    ], check=True)
    report = FOLDER / "board-geometry.json"
    mesh = FOLDER / "board.stl"
    subprocess.run([str(executable), str(args.step), str(report), str(mesh)], check=True)
    data = json.loads(report.read_text())
    data.update(step_sha256=digest, linear_deflection_mm=0.05, angular_deflection_rad=0.3)
    report.write_text(json.dumps(data, indent=2) + "\n")
    print(f"{report}\n{mesh}\nAssembly bounds: {data['bounds']}")


if __name__ == "__main__":
    main()

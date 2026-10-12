# 21. Fundamentals: documents, files and automation

Detailed specs for [catalog section 21](../solidworks-feature-catalog.md#21-fundamentals-documents-files-and-automation). See the [README](README.md) for sources and conventions.

## Document types

| Type | Extension | Template |
| --- | --- | --- |
| Part | `.sldprt` | `.prtdot` |
| Assembly | `.sldasm` | `.asmdot` |
| Drawing | `.slddrw` | `.drwdot`; sheet format `.slddrt` |

Related files: library features `.sldlfp`, blocks `.sldblk`, materials `.sldmat`, design tables (embedded `.xlsx`), curves `.sldcrv`, pattern tables `.sldptab`.

**OpenForge today:** parts only, as `.ofp`.

## New, open, save

Options: save as copy and continue, save as copy and open, save all, save as previous version, include description, references dialog when saving assemblies (rename referenced files), quick open (lightweight, large design review, detailing mode), open read-only, open with configuration and display state, and the references dialog in Open.

**OpenForge today:** new, open, save and save as; unsaved-changes prompt.

## Auto-recover and backup

Auto-recover saves every N minutes or N changes to a folder; on restart, the Document Recovery task pane offers recovered files. Backup keeps N previous saved versions in a folder.

## Document templates

Templates hold document properties (units, standards), custom properties, planes, materials and sheet formats. File Locations lists template folders; New dialog shows template tabs.

## Recent documents

List of recent files and folders with thumbnails, pinning and clearing.

## Pack and Go

Collects an assembly or drawing and every referenced file into a folder or zip. Options: include drawings, simulation results, Toolbox components, custom decals and appearances; flatten to one folder; add prefix or suffix to names; rename individual files; email after packing.

## Referenced files and external references

Find References lists every file an assembly or drawing uses with paths; search order for missing references; list, lock and break external references in parts; references update holders when files move.

## Print3D

Sends a part to a 3D printer or saves 3MF, AMF or STL with options: printer selection, build plate orientation and position, scale to fit, preview of overhangs and thin areas, units.

**OpenForge today:** STL export.

## Macros

Record VBA macros of UI actions, edit them in the VBA editor, run them from a toolbar button or keyboard shortcut; macro features embed code in the tree.

## API

COM API for VBA, VB.NET, C# and C++: documents, features (`IFeatureManager`), sketches, selections, custom properties, events, add-ins with their own CommandManager tabs and PropertyManager pages. The published reference ([help.solidworks.com API](https://help.solidworks.com/2025/english/api/sldworksapi/)) is the source of these specs.

**OpenForge today:** JSON-RPC protocol, CLI and MCP server cover a comparable programmable surface.

## Task Scheduler

Schedules batch tasks: update files to the new version, convert files to other formats, print, create drawings, import and export, run custom macros; with recurrence and time.

## Search

Searches files (Windows search), commands, community content and knowledge base from one box.

## Design Clipart

Extracts sketches, annotations and features from existing files into a clipart library to reuse.

## OLE objects

Embeds or links other documents (Word, Excel, images) into drawings and models.

## Multi-user environment

Detects when a file opened read-only has been saved by another user and offers reload; check read-only status; notify when write access becomes available.

## 3DEXPERIENCE platform integration

Saves files to the cloud platform with revisions, lifecycle states, collaborative spaces and markup. Out of scope for OpenForge.

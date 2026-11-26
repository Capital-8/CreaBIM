# Project information

The project's information is edited in the **Project** tab of Properties (the house icon at the foot of the tab column). *Manage › Project information* opens that tab. Every field is optional:

- **Project**: its name, the office's job number, the client, the status (*Planning application*, *Tender*…), the issue date, and the **Standard** the project draws to:
  - **ISO**: A0 to A4 sheets, scales written *1:50*.
  - **US**: ARCH A to E and ANSI A to E sheets, scales written *1/4" = 1'-0"* (1:48), *1/8" = 1'-0"* (1:96) and so on, and engineering scales up to *1" = 100'-0"*.

  The size and scale lists show the standard's own, with the others under *Show all*. Sheets write each view's scale and the title block's the standard's way. A new project takes the standard picked when CreaBIM was first set up. Changing it later keeps every sheet and view as it is.
- **Site**: its name, the postal address, the latitude and longitude, the project's zero above sea level, how far true north turns from the drawings' up, the month 3D shows the trees in, and where the project sits on the map.
- **Building**: its name and its use.

A field changes the project as you type, with no Apply: each field is one undo step. Grey words starting *e.g.* are examples, not values. A latitude or longitude reads *Not set* until you type one; clear it to unset it.

## Where it goes

- **Title blocks** read it. Fields with a small **sheet mark** at their right end are shown in title blocks: a title block's *job*, *project*, *client*, *address*, *status*, *issue_date*, *project_number*, *site* and *building* fields show these values on every sheet. A value set on one sheet still wins for that sheet; its *Sheet* tab shows the project's value in grey until you type its own.
- **IFC export** names the project, the site and the building from it, and places the site by its latitude, longitude and elevation.
- The **sun and north**: true north turns north arrows and boundary bearings; the latitude turns a tree's year south of the equator.

The starter house arrives with its project filled in, so its title blocks are complete.

# Project time

**Project time** counts the time you spend on a project while you work in it, splits it by **work stage** and by **what you were doing**, and shows each stage against its **fee** and **hours budget**, so you see early when a stage is running over. Everything stays on your computer: no account, nothing sent anywhere.

It is **off** until you switch it on for a project: *Project* tab › **Time and fees** › *Tracking*, or **Switch it on** in the Time area.

## What is counted

- **Working time**: the project is open, CreaBIM is in front and you are using it. This is the time that bills.
- **Open time**: the project is open but another program is in front (reading a brief, an email). Shown, not billed. After 30 minutes away it stops.
- **Idle**: no mouse or key for the *Idle after* time (5 minutes at first) stops the count until you come back. The pill says *Idle*.
- **Manual entries**, for work outside CreaBIM: a site visit, a meeting, a call. **Manual entry** in the Time area's header adds an hour ending now; set its day, times, stage and note in *Entries*.

*Tracking* **Auto** counts by itself. **Manual** counts only after you press **Start** in the pill's panel, as a stopwatch does.

## The time pill

While time is kept for a project, a pill at the left of the status bar shows today's working time and the stage it goes to. Its dot **beats** while time counts, in the stage's budget colour: green, **amber** from 80 % of its hours, **red** and faster past 100 %. It stops when paused or idle. *Pulse* in Time and fees turns the beat off; the system's *reduce motion* setting stops it too.

Click the pill to **Pause** or **Resume**, write a **Note** on what you are doing, switch the **work stage**, see this stage's hours against its budget and today's working, open and idle time, or open the Time area.

## The Time area

Pick **Time** in an area's view-type menu.

- **Stages**: each stage's fee, hours budget, hours spent and a bar of its use. Type a stage's fee and hours here; click a stage's name to make it the stage time goes to now; tick it when it is done.
- **Entries**: every stretch of work, newest first, by day, with its stage, who did it and a thin bar of how its time split between **modelling**, **drawing**, **sheets** and **other** (worked out from the view and tool in use: nothing to type). Click one to change its note or stage, or delete it.
- **Week**: hours per day, by stage, with the week's total and its value. The arrows step to earlier weeks.

## Stages, rates and timesheets

- **Stages** come from the project's drawing standard: the **RIBA Plan of Work** for A-sheet projects, the **AIA phases** for US ones. Pick **Own list** to name your own.
- **Rate** is your hourly rate: the value of the time, against a stage's fixed fee.
- **Timesheet CSV** and **Timesheet PDF**, in the Time area's header, write every entry with its stage and person, each stage's hours, budget and fee, and the totals. **Rounding** rounds each entry to 6, 15 or 30 minutes on them.
- **You** is the name this computer signs its time with, so a small studio sharing one project adds up correctly.

## Private by default

The time and fees are kept **beside** the project, in a file of the same name ending `.creabim-time`, never inside the `.creabim`. Sending the project to a client or contractor never sends your hours and rates, and an IFC export never carries them. Keep the two files together when you move a project.

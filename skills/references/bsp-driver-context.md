# BSP Driver Context

Use this when adding or debugging a board peripheral, chip driver, or board
support wrapper. Board facts are the authority for pins, buses, expanders,
power rails, reset lines, interrupts, and demo paths.

## Shape

A reusable driver context should separate:

- capability: what the board and chip can support;
- status: what the current firmware and hardware report;
- action: the smallest command or function that changes state;
- evidence: the output that proves the action worked.

## Source Matrix

Before writing driver code, collect:

- board source fact pack and completeness state;
- official board examples;
- chip vendor datasheet or driver docs;
- framework driver API docs;
- existing project code and public references.

If board-specific facts are incomplete, return the source-ingestion path instead
of presenting a generic driver recipe as runnable.

# Realization fixture

:::mara goal GOAL-EXPORT
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F00
:title: Share records
:status: accepted

Recipients can use exported records.
:::

:::mara requirement REQ-EXPORT
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F01
:title: Export records
:status: accepted
:kind: functional
:contributes_to: GOAL-EXPORT

Produce a record export when requested.
:::

:::mara design DES-EXPORT
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F02
:title: Export interface
:status: accepted
:kind: interface
:satisfies: REQ-EXPORT

The export command writes selected records to a file.
:::

:::mara verification VER-EXPORT
:mid: 01ARZ3NDEKTSV4RRFFQ69G5F03
:title: Check the export
:status: accepted
:method: test
:verifies: REQ-EXPORT

Invoke export with selected records and compare the output to those records.
:::

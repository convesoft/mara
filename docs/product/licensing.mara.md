# Licensing policy

:::goal m_01KY9JP4DZ8RE1T0DDNW83BKSS
:id: GOAL-OPEN-SOURCE-DISTRIBUTION
:title: Permit open-source use of Mara
:status: accepted
:kind: product
:priority: must

People and organizations shall be able to use, modify, and distribute Mara under
widely understood permissive open-source terms suitable for individual,
commercial, and collaborative development.
:::

:::req m_01KY9JQZC6EGV3NQ3NXB5PYAZH
:id: REQ-OPEN-SOURCE-LICENSE
:title: Mara distributions shall declare consistent dual licensing
:status: approved
:level: stakeholder
:kind: constraint
:priority: must
:derives_from: GOAL-OPEN-SOURCE-DISTRIBUTION

Every Mara source distribution and published Rust package shall declare the SPDX
license expression `MIT OR Apache-2.0`, include the complete MIT and Apache-2.0
license texts, and state the same licensing choice in its public documentation.
:::

:::decision m_01KY9JQZC7BJ819BG8G8SRMV5B
:id: ADR-0015
:title: License Mara under MIT OR Apache-2.0
:status: accepted
:kind: process
:justifies: REQ-OPEN-SOURCE-LICENSE

Mara is offered under the MIT License or the Apache License, Version 2.0, at the
recipient's option. This combines the concise and familiar MIT terms with the
explicit patent grant and contribution terms of Apache-2.0 without imposing a
single-license choice on users.

Unless explicitly stated otherwise, contributions intentionally submitted for
inclusion in Mara are accepted on the same dual-license basis.
:::

:::test m_01KY9JQZC977KAHQRB14P845PY
:id: TEST-OPEN-SOURCE-LICENSE
:title: Mara licensing declaration consistency inspection
:status: approved
:kind: verification
:method: inspection
:level: acceptance
:verifies: REQ-OPEN-SOURCE-LICENSE

Inspect a release candidate and fail the verification when the Cargo package
metadata does not declare `MIT OR Apache-2.0`, either complete license text is
missing, or the public licensing statement disagrees with the package metadata.
:::

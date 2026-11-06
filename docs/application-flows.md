# Aspirational application flows

[Application overview](application.md)

## Purpose and status

These proposed flows identify UI requirements before all services exist.
They are specifications for future applications, not descriptions of implemented screens.
The [shared UI standard](ui.md) remains authoritative for navigation, control behavior, and display updates.
The [UI requirements](platform/ui-requirements.md) assigns common requirements to the toolkit.

Application code owns workflows, retained models, validation, and service policy.
The toolkit owns reusable presentation and interaction mechanisms.
No transport, mesh protocol, Rust signature, or new backend capability is selected here.

## Common flow rules

Proposed Home destinations are Messages, Orienteering, Calculator, and Settings.
Settings contains Wi-Fi and Bluetooth destinations. Other applications can open these destinations through connection actions.
Back returns to the caller under the existing navigation contract.
Home returns to Home without clearing retained application state.

Drafts and selections survive page changes within an application session.
Unsubmitted passwords are an exception, as specified in the Wi-Fi flow.
Pending operations belong to application controllers, not visible controls.
Leaving an editor retains its draft. Cancel explicitly discards edits and restores the last applied value.
Leaving a discovery page cancels its scan. Leaving a pairing prompt rejects that pairing attempt.
Leaving an accepted connection or message request does not imply cancellation.
Returning to its page shows the controller's current state.

Destructive actions require explicit confirmation. Opening a destination never starts such an action.
Repeated activation while a request is pending must not submit another request.
Cancel requests remain distinct from confirmed cancellation.
Late results from canceled or replaced requests must not change the current operation.
Failures remain visible until retry, dismissal, or the specified workflow exit.

Durable retention requires the planned [storage services](services/storage.md).
Until durable storage exists, the UI must identify retained data as session-only.
Restart discards session-only data and clears pending UI state.
A future durable implementation must restore committed data and reconcile interrupted operations before offering retry.
It must not restore a pending label as evidence of an active operation.

Each collection and editor has an application-selected capacity.
A full collection rejects additions with a visible explanation and retains existing data.
An editor rejects excess input without changing accepted text.
Long essential text wraps or opens a detail view. It must not depend on clipped text for identification.

## Messaging

### Proposed flow

Messages opens a conversation list with peer names, previews, unread counts, and a New conversation action.
An empty list explains how to select a peer.
Peer selection opens a conversation with ordered message history and a Compose action.
Peer discovery and addressing remain transport dependencies. The UI must explain when no peer source exists.

Compose opens a multiline draft with recipient, remaining capacity, Send, and Cancel.
Send requires a recipient, nonempty text, and an available transport.
An unavailable transport leaves the draft editable and supplies a connection action when applicable.
Accepting Send creates one outgoing message and prevents duplicate submission of that draft.

History distinguishes Queued, Transmitted, Delivery confirmed, and Failed.
Queued means the transport accepted work. Transmitted means local transmission completed.
Only an explicit peer acknowledgment permits Delivery confirmed.
Transports without acknowledgments retain Transmitted as their final positive state.
An uncertain result displays Delivery unknown rather than Failed or Delivery confirmed.

A failed message offers Retry with the retained text and recipient.
Retry after an uncertain result requires confirmation because the peer could already have the message.
Protocol support for duplicate detection remains a service dependency.
Incoming messages preserve the reader's position. A new-message action moves to the latest message explicitly.

### Retention and dependencies

Back from Compose retains the draft. Back from the conversation returns to its caller and retains the reading position.
Home retains the same state. Connection loss retains history and drafts and updates outgoing status from service evidence.
Accepted outgoing work continues under its controller after page departure.
Committed history, drafts, and peer records are candidates for durable storage. Uncommitted edits remain session-only.

Current controls provide buttons, cards, badges, and append/delete text entry.
Message transport, peer management, acknowledgments, and durable conversation storage are unresolved dependencies.
See the [communications contract](services/connectivity.md#planned-communications-services).

## GPS and orienteering

### Proposed flow

Orienteering opens the current position, fix status, observation age, saved waypoints, and Save position.
Save position requires a fresh valid fix and opens an editor with copied coordinates and a name.
New waypoint permits manual WGS84 coordinate entry without a current fix.
Save validates coordinate ranges and required fields before adding or updating a waypoint.
Invalid input stays visible with a field-specific explanation.

Selecting a waypoint opens its details with Navigate, Edit, and Delete.
Delete requires confirmation. Deleting the active destination also stops navigation after confirmation.
Navigate shows destination, distance, true bearing, fix status, and Stop navigation.
Stop navigation ends guidance without deleting the waypoint.

No fix or a stale fix makes distance and bearing unavailable.
The page retains the destination and explains why guidance is unavailable.
Valid coordinates can provide true bearing to the destination without device orientation.
Course describes movement. It must not supply an arrow that claims to follow device orientation.
At the destination, an undefined bearing displays an unavailable marker.

Arrival is advisory and does not automatically stop navigation.
The application must select an arrival threshold and freshness limits before implementation.
Accuracy support and arrival reliability remain unresolved service dependencies.
Maps, route import, and track recording remain outside these flows.

### Retention and dependencies

Back and Home retain the selected destination and unfinished waypoint draft.
Provider interruption hides derived guidance until a fresh fix arrives. It does not remove saved waypoints.
Restart can restore committed waypoints when durable storage exists. Navigation requires an explicit start after restart.

The current [location service](services/location.md) supplies position and independent observation ages.
The application still needs waypoint storage, distance and bearing calculations, and arrival policy.
Accuracy and device orientation must not be inferred from the existing course value.

## Wi-Fi

### Proposed flow

Wi-Fi opens requested radio state, actual availability, current connection, saved networks, and Scan.
An enable request shows Pending until the service reports the resulting state.
An unavailable radio displays its reason separately from the requested state.
Scan displays access points with name, signal, and enough detail to distinguish duplicate names.
An empty successful scan offers Scan again. Failure and timeout use separate explanations.

Selecting an access point opens details and Connect.
A secured network opens a password editor with concealment, explicit reveal, Connect, and Cancel.
An open network requires no password. Unsupported authentication disables Connect with a reason.
Pending connection shows its target and a cancellation action when supported.
Authentication failure preserves the target and returns to credential editing.

Success shows the connected network and Disconnect.
Network association does not prove internet access.
Internet status stays Unknown unless a separate service supplies evidence. Negative evidence can show Connected without internet.
Connection loss preserves the selected network and offers Reconnect.
This proposal uses explicit reconnect and does not select an automatic reconnect policy.

Disconnect requests connection teardown and waits for observed completion.
Forget requires confirmation and removes saved credentials only after the settings operation succeeds.
For an active network, Forget first requests disconnection. Failure retains the saved entry and explains the remaining state.

### Retention and dependencies

Back and Home cancel page-owned scans but leave accepted connection requests under controller ownership.
Departure clears an unsubmitted password. Passwords never appear in ordinary status text.
Credential retention across restart requires a backend storage policy. It is not supplied by a generic text field.
Restart reads actual radio state instead of assuming that a saved preference took effect.

Current [discovery](services/connectivity.md#current-discovery-interface) supports scans, cancellation, and access-point identities.
Authentication metadata, radio control, connections, credential storage, and internet observations remain unresolved dependencies.

## Bluetooth

### Proposed accessory flow

Bluetooth opens availability, paired devices, active connections, Find accessory, and Pair with phone.
Find accessory starts discovery. Selecting a result opens identity, supported functions, and Connect.
An unknown function shows Unknown. An unsupported function disables its action with an explanation.
Names alone must not identify a device when multiple results have the same name.

Connect requests pairing only when required by the selected function and backend.
A pairing page shows the remote identity and the confirmation or entry method required by the service.
Reject and timeout end the attempt without reporting a connection.
Successful pairing creates a paired relationship. It does not prove an active connection.
The device detail page separately shows pairing state, connection state, and available functions.

### Proposed phone flow

Pair with phone explicitly starts a bounded discoverable period and shows Stop discovery.
Leaving this page ends the discoverable period, even when no pairing request has arrived.
An incoming request opens a pairing prompt with the remote identity and required confirmation.
Back, Home, Reject, or timeout rejects the pending request and ends this discoverable period.
An unsolicited request must not approve itself or interrupt another modal operation silently.
The controller rejects requests that the UI cannot present safely.
After pairing, the page waits for a separately reported companion connection.

### Retention and dependencies

Disconnect ends an active connection without deleting the paired relationship.
Forget requires confirmation, disconnects when necessary, and removes the relationship after backend confirmation.
Failure preserves the last confirmed state and permits retry.
Connection loss leaves the paired relationship visible and offers explicit Reconnect.
Back and Home cancel discovery and pending pairing. Established connections remain active.
Restart reconstructs paired relationships from backend storage and reads actual connection state.

Current discovery covers Bluetooth LE observations only.
Accessory functions, companion protocol, pairing methods, discoverability, connections, and durable relationships remain unresolved dependencies.
Both roles depend on reported capabilities. This proposal does not promise Bluetooth Classic or simultaneous roles.

## Basic calculator

The initial calculator is now implemented. See [current calculator behavior](application.md#calculator) for arithmetic limits and retained state.
The flow below records its original design intent. The other application flows remain proposed work.

### Proposed flow

Calculator opens a numeric display and keys for digits, decimal point, sign, four arithmetic operations, delete, clear, and equals.
The first implementation uses sequential arithmetic: each new operator resolves the previous complete operation.
For example, `2 + 3 × 4 =` produces `20`.
An operator before a second operand replaces the pending operator.
Equals with an incomplete operation shows an explanation and preserves editable input.
Repeated equals does not repeat the previous operation.

Sign changes the current operand. Delete removes its last input character.
Clear resets operands, result, pending operator, and errors.
Division by zero and numeric limits show explicit errors without displaying a fabricated result.
Following an error, Clear resets the calculation. A digit starts a new calculation.
The application must select decimal precision and numeric limits before implementation and display input capacity when reached.

### Retention and dependencies

Back and Home retain the current calculation during the session. Restart opens a cleared calculator.
The cleared calculator displays zero and accepts the first operand. It has no separate loading or unavailable state.
No external service is required. Service interruptions must not affect arithmetic.
Existing buttons can supply actions, but a reusable keypad layout and numeric display are missing.
Arithmetic and decimal policy belong to the application, not the keypad.

## System settings

### Proposed flow

Settings opens Display, Connectivity, Time, Units, and Device information.
Category rows show the current value or a reason that a setting is unavailable.
Connectivity opens the Wi-Fi and Bluetooth flows above.
Device information uses read-only rows. Other controls appear only for supported capabilities.

An editable setting opens its current value, help, proposed value, Apply, and Cancel.
Changes remain drafts until Apply. Validation errors keep the draft and identify the affected field.
Apply shows Pending until the service reports completion.
For durable settings, success also requires a confirmed settings commit.
If runtime application succeeds but persistence fails, the page shows the active value and the unsaved state separately.
Retry must not repeat an already completed runtime action unnecessarily.

Cancel restores the last applied value and discards the draft.
Back and Home retain unapplied drafts with an Unsaved label on return.
Restore defaults opens a confirmation that identifies the category and affected values.
Confirmation uses the same application and persistence states as ordinary edits.
It must report partial success per setting when the backend cannot apply the category atomically.

### Retention and dependencies

Pending application continues under the controller after navigation.
Service interruption keeps the draft and last confirmed value. It does not claim that the requested value became active.
Restart restores committed preferences where supported and discards drafts.
The page compares those preferences with actual service state before displaying them as active.

Current UI controls and service observations can display information.
Writable capability contracts and durable [settings storage](services/storage.md) remain dependencies for editable settings.
Display, time, and connectivity options require individual service support. Their presence in this proposal does not establish that support.

## Deterministic acceptance scenarios

These scenarios are proposed simulator coverage, not tests that currently exist.
Fixtures must control capabilities, observations, completion order, and time.
They must use simulated credentials and peers, with no external network requirement.

| Flow | Controlled sequence | Expected observation |
| --- | --- | --- |
| Messaging | Send once, tap again while pending, report transmission, then omit acknowledgment. | One request exists. History shows Transmitted, never Delivery confirmed. |
| Messaging | Edit a draft, press Back and Home, reopen, then lose the transport. | Draft survives. Send becomes unavailable with a reason. |
| Orienteering | Select a waypoint, provide a fix, expire it, then provide a fresh fix. | Destination survives. Guidance disappears and then resumes. |
| Orienteering | Enter an invalid latitude, then use the exact destination as the fix. | Save rejects the invalid value. At the destination, undefined bearing stays unavailable. |
| Wi-Fi | Return duplicate names, reject a password, retry, then report association without internet evidence. | Identities remain distinct. Status reports connection with internet status Unknown. |
| Wi-Fi | Leave during a scan, start another scan, then deliver the old completion. | Old results do not replace the new scan. |
| Bluetooth | Pair successfully, delay connection, disconnect, then reconnect. | Paired and Connected remain separate states throughout. |
| Bluetooth | Receive a phone pairing request, press Home, then deliver late success. | The canceled attempt cannot become the active UI session. |
| Calculator | Enter `2 + 3 × 4 =`, press equals again, then divide by zero. | Result stays `20` after repeated equals. Division by zero shows an error. |
| Settings | Apply a value, complete runtime application, fail persistence, then retry persistence. | Active and unsaved states remain distinct. Retry does not repeat the runtime action. |

For every flow, also exercise long labels, empty content, full capacity, repeated input, Back, and Home.
Restart scenarios must distinguish session-only data from confirmed durable data.
Interrupt each pending service operation and verify that the page offers only actions supported by its resulting state.
Physical readability and touch accuracy still require the device checks in the shared UI standard.

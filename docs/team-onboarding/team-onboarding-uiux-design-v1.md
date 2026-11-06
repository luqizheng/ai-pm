# PM-AI — Team Onboarding UI/UX Design

> 关联基线：产品裁决 2026-11-06（team-onboarding-team-management-spec-v1.md）、技术基线 V1.0（team-domain-model-database-design-v1.md）。

## 1. Project Context

Design the Team Onboarding experience for PM-AI, an AI-powered R&D and Project Management Platform.

This module is the mandatory entry point for users who have logged in but do not yet belong to any Team.

A Team is the organizational boundary of PM-AI. Projects belong to Teams, not directly to individual users.

A user can belong to multiple Teams. One Team can have multiple users.

## 2. Product Rules — Must Follow

1. A newly registered or logged-in user with no active Team Membership MUST complete Team Onboarding.
2. There is no Skip or Later option.
3. Users can either:
   - Create a new Team.
   - Join an existing Team through an invitation.
4. The creator automatically becomes the Team OWNER.
5. Team invitations support:
   - Email invitation.
   - Invitation link.
6. Joining a Team requires explicit user confirmation.
7. A user can belong to multiple Teams.
8. Do not introduce a generic Workspace concept.
9. Do not treat Team as a Project.
10. Do not allow users to create Projects before joining or creating a Team.

## 3. Visual Design Direction

Use the existing PM-AI Design System:

- Slate & Muted Teal visual identity.
- Professional enterprise SaaS appearance.
- Light and Dark themes.
- Bright, clean light theme.
- Restrained teal accents.
- Neutral slate surfaces.
- Clear typography and compact information density.
- Consistent spacing, borders, radii, shadows and form components.

Avoid:

- Purple brand accents.
- Excessive gradients.
- Oversized decorative illustrations.
- Consumer social-media style.
- Unnecessary onboarding animations.

## 4. Required Screens

### Screen 01 — Welcome / Team Selection

Design a first-login welcome page.

Headline:

"Welcome to PM-AI"

Description:

"Create your team or join an existing team to start managing your projects."

Two prominent choices:

A. Create a Team

- Icon: Building / Users
- Description: Set up a new team and invite your colleagues.

B. Join a Team

- Icon: User Plus
- Description: Join a team using an invitation.

This page must not contain Skip or Later.

### Screen 02 — Create Team

Create a clean team creation form.

Fields:

- Team Name (required)
- Team Slug (optional, suggested automatically)
- Description (optional)

Display:

- Creator will automatically become Team Owner.
- Explain that team members and projects will be managed under this Team.

Actions:

- Create Team
- Cancel / Back

Validation:

- Required name.
- Duplicate slug.
- Invalid characters.
- Network error.
- Creation failure.

### Screen 03 — Team Created Successfully

Display:

- Team name.
- Success confirmation.
- Current user role: OWNER.
- Next step: Invite team members.

Actions:

- Invite Members
- Continue to Team Dashboard

### Screen 04 — Invite Team Members

Support inviting multiple users.

Fields:

- Email address.
- Role selector.
- Add another email.

Available roles:

- ADMIN
- MEMBER

OWNER must not be assignable through the normal invitation interface.

Show a compact invitation list:

- Email
- Assigned role
- Remove invitation recipient

Actions:

- Send Invitations
- Skip Invitations / Continue

Allow the team creator to continue without inviting anyone.

### Screen 05 — Invitation Link

Provide an invitation-link generation interface.

Fields:

- Default role (MEMBER; ADMIN only when authorized).
- Expiration.
- Maximum number of uses.
- Unlimited uses option.

Display:

- Generated invitation link.
- Copy link action.
- Link expiration.
- Usage information.
- Revoke link action.

The link must not automatically add someone to the Team. It only initiates the invitation acceptance process.

### Screen 06 — Join Team / Accept Invitation

Design the invitation acceptance page.

Display:

- Team name.
- Team logo or initials.
- Inviter.
- Assigned role.
- Invitation type.
- Expiration information.

Actions:

- Accept Invitation
- Decline Invitation

If not logged in, request login or registration first.

For email invitations, show a clear email ownership verification state.

For invitation links, require login and explicit confirmation.

### Screen 07 — Invitation Result

Design success and exception states.

Success:

- Successfully joined Team.
- Team name.
- Assigned role.
- Continue to Team Dashboard.

Exceptions:

- Invitation expired.
- Invitation revoked.
- Invalid invitation.
- Already a member.
- Invitation email mismatch.
- Team suspended.
- Invitation link exhausted.

Provide clear next actions for each exception.

## 5. Navigation Rules

During onboarding:

- Hide normal Project navigation.
- Do not show Team management pages before membership exists.
- Do not expose unrelated platform functionality.

After successful creation or joining:

- Enter the Team Dashboard.
- Show the current Team in the global Team Switcher.

## 6. Interaction Requirements

Create realistic interactive prototypes for:

1. First login → Create Team.
2. First login → Join Team.
3. Create Team → Invite Members.
4. Create Team → Generate Invitation Link.
5. Accept Email Invitation.
6. Accept Invitation Link.
7. Invitation expiration and failure handling.

Use consistent form validation and confirmation dialogs.

## 7. Deliverables

Produce:

- High-fidelity desktop UI.
- Light theme.
- Dark theme.
- Reusable components.
- Interaction prototypes.
- Loading, empty, success and error states.

Design desktop first, with responsive tablet and mobile adaptations.

The output must be implementation-ready and consistent with the existing PM-AI component library.
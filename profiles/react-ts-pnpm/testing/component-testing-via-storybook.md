# Storybook as Visual Support, Not Behavior Authority

Storybook remains required for component documentation, visual states, interaction exploration, and accessibility checks. Behavior proof comes from Playwright+Cucumber scenarios.

```tsx
// Storybook remains valuable for visual contracts and a11y checks.
export const Disabled: Story = {
  args: { disabled: true, children: "Submit" },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement);
    await expect(canvas.getByRole("button")).toBeDisabled();
  },
};
```

```gherkin
@behavior(BEH-AUTH-021) @tier(integration)
Scenario: User cannot submit disabled login form
  Given the login form is disabled
  When the user attempts submission
  Then submission is blocked and an explanatory message is shown
```

**Rules:**
- Storybook is not the source of truth for shipped behavior
- Behavior claims require `.feature` scenarios
- Keep Storybook stories and BDD scenarios aligned for major UI states

**Why:** Storybook is great for visual and interaction confidence, but user behavior proof belongs to executable scenarios.

import { mount, unmount } from 'svelte';
import { expect } from 'vitest';

type MountedComponent = {
  app: Record<string, any>;
  target: HTMLElement;
};

const mountedComponents: MountedComponent[] = [];

export function renderComponent(component: any, props: Record<string, unknown> = {}) {
  const target = document.createElement('div');
  document.body.append(target);

  const app = mount(component, { target, props }) as Record<string, any>;
  mountedComponents.push({ app, target });

  return target;
}

export function byTestId(target: ParentNode, testId: string) {
  const element = target.querySelector(`[data-testid="${testId}"]`);

  expect(element).toBeInstanceOf(HTMLElement);

  return element as HTMLElement;
}

export async function cleanupRenderedComponents() {
  for (const { app, target } of mountedComponents.splice(0).reverse()) {
    await unmount(app);
    target.remove();
  }
}

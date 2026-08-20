export type PhaseZeroInput = {
  readonly scenario:
    | 'happy_path'
    | 'missing_element'
    | 'authentication_challenge'
    | 'portal_failure'
    | 'runtime_failure'
    | 'ambiguous_post_side_effect';
};

export type PhaseZeroOutput = {
  readonly scenario: 'happy_path';
  readonly status: 'completed';
  readonly receipt: string;
};

export function readInput(value: unknown): PhaseZeroInput {
  if (!value || typeof value !== 'object' || Array.isArray(value)) {
    throw new Error('INPUT_VALIDATION: input must be an object');
  }
  const scenario = (value as { scenario?: unknown }).scenario;
  const supported = [
    'happy_path',
    'missing_element',
    'authentication_challenge',
    'portal_failure',
    'runtime_failure',
    'ambiguous_post_side_effect',
  ];
  if (typeof scenario !== 'string' || !supported.includes(scenario)) {
    throw new Error('INPUT_VALIDATION: scenario is not supported');
  }
  return { scenario: scenario as PhaseZeroInput['scenario'] };
}

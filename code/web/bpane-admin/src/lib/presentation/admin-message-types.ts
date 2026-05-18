export type AdminMessageVariant = 'info' | 'success' | 'warning' | 'error' | 'loading' | 'empty';

export type AdminMessageFeedback = {
  readonly variant: AdminMessageVariant;
  readonly message: string;
  readonly title?: string;
  readonly testId?: string;
};

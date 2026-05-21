export const SESSION_NOT_FOUND_PARAM = "sessionNotFound";

export function sessionNotFoundHomePath(): string {
  return `/?${SESSION_NOT_FOUND_PARAM}=1`;
}

// index.d.d.ts
export interface ShowT {}
export interface HideT {}
export class SpecializedComponent extends SomeComponent {
  override show(): ShowT;
  override hide(): HideT;
}

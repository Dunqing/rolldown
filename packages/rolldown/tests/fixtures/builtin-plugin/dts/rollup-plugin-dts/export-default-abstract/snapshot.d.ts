// index.d.d.ts
export default interface MemberTypes {}
export default interface TypeInfo {}
export default abstract class MemberInfo {
  abstract readonly name: string;
  abstract readonly declaringType: TypeInfo;
  abstract readonly memberType: MemberTypes;
}

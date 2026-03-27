// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
import TestSchema

public struct PetDetails: TestSchema.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment PetDetails on Pet { __typename humanName favoriteToy owner { __typename firstName } }"#
  }

  public let __data: DataDict
  public init(_dataDict: DataDict) { __data = _dataDict }

  public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.Pet }
  public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("humanName", String?.self),
    .field("favoriteToy", String.self),
    .field("owner", Owner?.self),
  ] }

  public var humanName: String? { __data["humanName"] }
  public var favoriteToy: String { __data["favoriteToy"] }
  public var owner: Owner? { __data["owner"] }

  /// Owner
  public struct Owner: TestSchema.SelectionSet {
    public let __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Human }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("firstName", String.self),
    ] }

    public var firstName: String { __data["firstName"] }
  }
}

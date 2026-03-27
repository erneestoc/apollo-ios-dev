// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
import TestSchema

public struct HeightInMeters: TestSchema.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment HeightInMeters on Animal { __typename height { __typename meters } }"#
  }

  public let __data: DataDict
  public init(_dataDict: DataDict) { __data = _dataDict }

  public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.RenamedAnimal }
  public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .field("height", Height.self),
  ] }

  public var height: Height { __data["height"] }

  /// Height
  public struct Height: TestSchema.SelectionSet {
    public let __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Height }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("__typename", String.self),
      .field("meters", Int.self),
    ] }

    public var meters: Int { __data["meters"] }
  }
}

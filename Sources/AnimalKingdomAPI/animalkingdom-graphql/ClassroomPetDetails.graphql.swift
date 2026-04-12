// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
import TestSchema

public struct ClassroomPetDetails: TestSchema.SelectionSet, Fragment {
  public static var fragmentDefinition: StaticString {
    #"fragment ClassroomPetDetails on ClassroomPet { __typename ... on Animal { species } ... on Pet { humanName } ... on WarmBlooded { laysEggs } ... on Cat { bodyTemperature isJellicle } ... on Bird { wingspan } ... on PetRock { favoriteToy } }"#
  }

  public let __data: DataDict
  public init(_dataDict: DataDict) { __data = _dataDict }

  public static var __parentType: any ApolloAPI.ParentType { TestSchema.Unions.ClassroomPet }
  public static var __selections: [ApolloAPI.Selection] { [
    .field("__typename", String.self),
    .inlineFragment(AsRenamedAnimal.self),
    .inlineFragment(AsPet.self),
    .inlineFragment(AsWarmBlooded.self),
    .inlineFragment(AsCat.self),
    .inlineFragment(AsBird.self),
    .inlineFragment(AsPetRock.self),
  ] }

  public var asRenamedAnimal: AsRenamedAnimal? { _asInlineFragment() }
  public var asPet: AsPet? { _asInlineFragment() }
  public var asWarmBlooded: AsWarmBlooded? { _asInlineFragment() }
  public var asCat: AsCat? { _asInlineFragment() }
  public var asBird: AsBird? { _asInlineFragment() }
  public var asPetRock: AsPetRock? { _asInlineFragment() }

  /// AsRenamedAnimal
  public struct AsRenamedAnimal: TestSchema.InlineFragment {
    public let __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public typealias RootEntityType = ClassroomPetDetails
    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.RenamedAnimal }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("species", String.self),
    ] }

    public var species: String { __data["species"] }
  }

  /// AsPet
  public struct AsPet: TestSchema.InlineFragment {
    public let __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public typealias RootEntityType = ClassroomPetDetails
    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.Pet }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("humanName", String?.self),
    ] }

    public var humanName: String? { __data["humanName"] }
  }

  /// AsWarmBlooded
  public struct AsWarmBlooded: TestSchema.InlineFragment {
    public let __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public typealias RootEntityType = ClassroomPetDetails
    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.WarmBlooded }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("laysEggs", Bool.self),
    ] }

    public var laysEggs: Bool { __data["laysEggs"] }
    public var species: String { __data["species"] }
  }

  /// AsCat
  public struct AsCat: TestSchema.InlineFragment {
    public let __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public typealias RootEntityType = ClassroomPetDetails
    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Cat }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("bodyTemperature", Int.self),
      .field("isJellicle", Bool.self),
    ] }

    public var bodyTemperature: Int { __data["bodyTemperature"] }
    public var isJellicle: Bool { __data["isJellicle"] }
    public var species: String { __data["species"] }
    public var humanName: String? { __data["humanName"] }
    public var laysEggs: Bool { __data["laysEggs"] }
  }

  /// AsBird
  public struct AsBird: TestSchema.InlineFragment {
    public let __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public typealias RootEntityType = ClassroomPetDetails
    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Bird }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("wingspan", Double.self),
    ] }

    public var wingspan: Double { __data["wingspan"] }
    public var species: String { __data["species"] }
    public var humanName: String? { __data["humanName"] }
    public var laysEggs: Bool { __data["laysEggs"] }
  }

  /// AsPetRock
  public struct AsPetRock: TestSchema.InlineFragment {
    public let __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public typealias RootEntityType = ClassroomPetDetails
    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.PetRock }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("favoriteToy", String.self),
    ] }

    public var favoriteToy: String { __data["favoriteToy"] }
    public var humanName: String? { __data["humanName"] }
  }
}

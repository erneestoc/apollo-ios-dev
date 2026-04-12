// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
import TestSchema

public class AllAnimalsQuery: GraphQLQuery {
  public static let operationName: String = "AllAnimalsQuery"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query AllAnimalsQuery { allAnimals { __typename id height { __typename feet inches } ...HeightInMeters ...WarmBloodedDetails species skinCovering ... on Pet { ...PetDetails ...WarmBloodedDetails ... on Animal { height { __typename relativeSize centimeters } } } ... on Cat { isJellicle } ... on ClassroomPet { ... on Bird { wingspan } } ... on Dog { favoriteToy birthdate } predators { __typename species ... on WarmBlooded { predators { __typename species } ...WarmBloodedDetails laysEggs } } } }"#,
      fragments: [HeightInMeters.self, PetDetails.self, WarmBloodedDetails.self]
    ))

  public init() {}

  public struct Data: TestSchema.SelectionSet {
    public let __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Query }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("allAnimals", [AllAnimal].self),
    ] }

    public var allAnimals: [AllAnimal] { __data["allAnimals"] }

    /// AllAnimal
    public struct AllAnimal: TestSchema.SelectionSet {
      public let __data: DataDict
      public init(_dataDict: DataDict) { __data = _dataDict }

      public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.RenamedAnimal }
      public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .field("id", TestSchema.ID.self),
        .field("height", Height.self),
        .field("species", String.self),
        .field("skinCovering", GraphQLEnum<TestSchema.SkinCovering>?.self),
        .field("predators", [Predator].self),
        .inlineFragment(AsWarmBlooded.self),
        .inlineFragment(AsPet.self),
        .inlineFragment(AsCat.self),
        .inlineFragment(AsClassroomPet.self),
        .inlineFragment(AsDog.self),
        .fragment(HeightInMeters.self),
      ] }

      public var id: TestSchema.ID { __data["id"] }
      public var height: Height { __data["height"] }
      public var species: String { __data["species"] }
      public var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
      public var predators: [Predator] { __data["predators"] }

      public var asWarmBlooded: AsWarmBlooded? { _asInlineFragment() }
      public var asPet: AsPet? { _asInlineFragment() }
      public var asCat: AsCat? { _asInlineFragment() }
      public var asClassroomPet: AsClassroomPet? { _asInlineFragment() }
      public var asDog: AsDog? { _asInlineFragment() }

      public struct Fragments: FragmentContainer {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public var heightInMeters: HeightInMeters { _toFragment() }
      }

      /// AllAnimal.Height
      public struct Height: TestSchema.SelectionSet {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Height }
        public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("feet", Int.self),
          .field("inches", Int?.self),
        ] }

        public var feet: Int { __data["feet"] }
        public var inches: Int? { __data["inches"] }
        public var meters: Int { __data["meters"] }
      }

      /// AllAnimal.Predator
      public struct Predator: TestSchema.SelectionSet {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.RenamedAnimal }
        public static var __selections: [ApolloAPI.Selection] { [
          .field("__typename", String.self),
          .field("species", String.self),
          .inlineFragment(AsWarmBlooded.self),
        ] }

        public var species: String { __data["species"] }

        public var asWarmBlooded: AsWarmBlooded? { _asInlineFragment() }

        /// AllAnimal.Predator.AsWarmBlooded
        public struct AsWarmBlooded: TestSchema.InlineFragment {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal.Predator
          public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.WarmBlooded }
          public static var __selections: [ApolloAPI.Selection] { [
            .field("predators", [Predator].self),
            .field("laysEggs", Bool.self),
            .fragment(WarmBloodedDetails.self),
          ] }

          public var predators: [Predator] { __data["predators"] }
          public var laysEggs: Bool { __data["laysEggs"] }
          public var species: String { __data["species"] }
          public var bodyTemperature: Int { __data["bodyTemperature"] }
          public var height: Height { __data["height"] }

          public struct Fragments: FragmentContainer {
            public let __data: DataDict
            public init(_dataDict: DataDict) { __data = _dataDict }

            public var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
            public var heightInMeters: HeightInMeters { _toFragment() }
          }

          /// AllAnimal.Predator.AsWarmBlooded.Predator
          public struct Predator: TestSchema.SelectionSet {
            public let __data: DataDict
            public init(_dataDict: DataDict) { __data = _dataDict }

            public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.RenamedAnimal }
            public static var __selections: [ApolloAPI.Selection] { [
              .field("__typename", String.self),
              .field("species", String.self),
            ] }

            public var species: String { __data["species"] }
          }

          public typealias Height = HeightInMeters.Height
        }
      }

      /// AllAnimal.AsWarmBlooded
      public struct AsWarmBlooded: TestSchema.InlineFragment {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.WarmBlooded }
        public static var __selections: [ApolloAPI.Selection] { [
          .fragment(WarmBloodedDetails.self),
        ] }

        public var id: TestSchema.ID { __data["id"] }
        public var height: Height { __data["height"] }
        public var species: String { __data["species"] }
        public var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        public var predators: [Predator] { __data["predators"] }
        public var bodyTemperature: Int { __data["bodyTemperature"] }

        public struct Fragments: FragmentContainer {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
          public var heightInMeters: HeightInMeters { _toFragment() }
        }

        /// AllAnimal.AsWarmBlooded.Height
        public struct Height: TestSchema.SelectionSet {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Height }

          public var feet: Int { __data["feet"] }
          public var inches: Int? { __data["inches"] }
          public var meters: Int { __data["meters"] }
        }
      }

      /// AllAnimal.AsPet
      public struct AsPet: TestSchema.InlineFragment {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.Pet }
        public static var __selections: [ApolloAPI.Selection] { [
          .field("height", Height.self),
          .inlineFragment(AsWarmBlooded.self),
          .fragment(PetDetails.self),
        ] }

        public var height: Height { __data["height"] }
        public var id: TestSchema.ID { __data["id"] }
        public var species: String { __data["species"] }
        public var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        public var predators: [Predator] { __data["predators"] }
        public var humanName: String? { __data["humanName"] }
        public var favoriteToy: String { __data["favoriteToy"] }
        public var owner: Owner? { __data["owner"] }

        public var asWarmBlooded: AsWarmBlooded? { _asInlineFragment() }

        public struct Fragments: FragmentContainer {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public var petDetails: PetDetails { _toFragment() }
          public var heightInMeters: HeightInMeters { _toFragment() }
        }

        /// AllAnimal.AsPet.Height
        public struct Height: TestSchema.SelectionSet {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Height }
          public static var __selections: [ApolloAPI.Selection] { [
            .field("__typename", String.self),
            .field("relativeSize", GraphQLEnum<TestSchema.RelativeSize>.self),
            .field("centimeters", Double.self),
          ] }

          public var relativeSize: GraphQLEnum<TestSchema.RelativeSize> { __data["relativeSize"] }
          public var centimeters: Double { __data["centimeters"] }
          public var feet: Int { __data["feet"] }
          public var inches: Int? { __data["inches"] }
          public var meters: Int { __data["meters"] }
        }

        public typealias Owner = PetDetails.Owner

        /// AllAnimal.AsPet.AsWarmBlooded
        public struct AsWarmBlooded: TestSchema.InlineFragment {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
          public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.WarmBlooded }
          public static var __selections: [ApolloAPI.Selection] { [
            .fragment(WarmBloodedDetails.self),
          ] }

          public var id: TestSchema.ID { __data["id"] }
          public var height: Height { __data["height"] }
          public var species: String { __data["species"] }
          public var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
          public var predators: [Predator] { __data["predators"] }
          public var bodyTemperature: Int { __data["bodyTemperature"] }
          public var humanName: String? { __data["humanName"] }
          public var favoriteToy: String { __data["favoriteToy"] }
          public var owner: Owner? { __data["owner"] }

          public struct Fragments: FragmentContainer {
            public let __data: DataDict
            public init(_dataDict: DataDict) { __data = _dataDict }

            public var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
            public var heightInMeters: HeightInMeters { _toFragment() }
            public var petDetails: PetDetails { _toFragment() }
          }

          /// AllAnimal.AsPet.AsWarmBlooded.Height
          public struct Height: TestSchema.SelectionSet {
            public let __data: DataDict
            public init(_dataDict: DataDict) { __data = _dataDict }

            public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Height }

            public var feet: Int { __data["feet"] }
            public var inches: Int? { __data["inches"] }
            public var meters: Int { __data["meters"] }
            public var relativeSize: GraphQLEnum<TestSchema.RelativeSize> { __data["relativeSize"] }
            public var centimeters: Double { __data["centimeters"] }
          }

          public typealias Owner = PetDetails.Owner
        }
      }

      /// AllAnimal.AsCat
      public struct AsCat: TestSchema.InlineFragment {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Cat }
        public static var __selections: [ApolloAPI.Selection] { [
          .field("isJellicle", Bool.self),
        ] }

        public var isJellicle: Bool { __data["isJellicle"] }
        public var id: TestSchema.ID { __data["id"] }
        public var height: Height { __data["height"] }
        public var species: String { __data["species"] }
        public var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        public var predators: [Predator] { __data["predators"] }
        public var bodyTemperature: Int { __data["bodyTemperature"] }
        public var humanName: String? { __data["humanName"] }
        public var favoriteToy: String { __data["favoriteToy"] }
        public var owner: Owner? { __data["owner"] }

        public struct Fragments: FragmentContainer {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public var heightInMeters: HeightInMeters { _toFragment() }
          public var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
          public var petDetails: PetDetails { _toFragment() }
        }

        /// AllAnimal.AsCat.Height
        public struct Height: TestSchema.SelectionSet {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Height }

          public var feet: Int { __data["feet"] }
          public var inches: Int? { __data["inches"] }
          public var meters: Int { __data["meters"] }
          public var relativeSize: GraphQLEnum<TestSchema.RelativeSize> { __data["relativeSize"] }
          public var centimeters: Double { __data["centimeters"] }
        }

        public typealias Owner = PetDetails.Owner
      }

      /// AllAnimal.AsClassroomPet
      public struct AsClassroomPet: TestSchema.InlineFragment {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Unions.ClassroomPet }
        public static var __selections: [ApolloAPI.Selection] { [
          .inlineFragment(AsBird.self),
        ] }

        public var id: TestSchema.ID { __data["id"] }
        public var height: Height { __data["height"] }
        public var species: String { __data["species"] }
        public var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        public var predators: [Predator] { __data["predators"] }

        public var asBird: AsBird? { _asInlineFragment() }

        public struct Fragments: FragmentContainer {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public var heightInMeters: HeightInMeters { _toFragment() }
        }

        /// AllAnimal.AsClassroomPet.Height
        public struct Height: TestSchema.SelectionSet {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Height }

          public var feet: Int { __data["feet"] }
          public var inches: Int? { __data["inches"] }
          public var meters: Int { __data["meters"] }
        }

        /// AllAnimal.AsClassroomPet.AsBird
        public struct AsBird: TestSchema.InlineFragment {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
          public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Bird }
          public static var __selections: [ApolloAPI.Selection] { [
            .field("wingspan", Double.self),
          ] }

          public var wingspan: Double { __data["wingspan"] }
          public var id: TestSchema.ID { __data["id"] }
          public var height: Height { __data["height"] }
          public var species: String { __data["species"] }
          public var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
          public var predators: [Predator] { __data["predators"] }
          public var bodyTemperature: Int { __data["bodyTemperature"] }
          public var humanName: String? { __data["humanName"] }
          public var favoriteToy: String { __data["favoriteToy"] }
          public var owner: Owner? { __data["owner"] }

          public struct Fragments: FragmentContainer {
            public let __data: DataDict
            public init(_dataDict: DataDict) { __data = _dataDict }

            public var heightInMeters: HeightInMeters { _toFragment() }
            public var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
            public var petDetails: PetDetails { _toFragment() }
          }

          /// AllAnimal.AsClassroomPet.AsBird.Height
          public struct Height: TestSchema.SelectionSet {
            public let __data: DataDict
            public init(_dataDict: DataDict) { __data = _dataDict }

            public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Height }

            public var feet: Int { __data["feet"] }
            public var inches: Int? { __data["inches"] }
            public var meters: Int { __data["meters"] }
            public var relativeSize: GraphQLEnum<TestSchema.RelativeSize> { __data["relativeSize"] }
            public var centimeters: Double { __data["centimeters"] }
          }

          public typealias Owner = PetDetails.Owner
        }
      }

      /// AllAnimal.AsDog
      public struct AsDog: TestSchema.InlineFragment {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = AllAnimalsQuery.Data.AllAnimal
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Dog }
        public static var __selections: [ApolloAPI.Selection] { [
          .field("favoriteToy", String.self),
          .field("birthdate", TestSchema.CustomDate?.self),
        ] }

        public var favoriteToy: String { __data["favoriteToy"] }
        public var birthdate: TestSchema.CustomDate? { __data["birthdate"] }
        public var id: TestSchema.ID { __data["id"] }
        public var height: Height { __data["height"] }
        public var species: String { __data["species"] }
        public var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        public var predators: [Predator] { __data["predators"] }
        public var bodyTemperature: Int { __data["bodyTemperature"] }
        public var humanName: String? { __data["humanName"] }
        public var owner: Owner? { __data["owner"] }

        public struct Fragments: FragmentContainer {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public var heightInMeters: HeightInMeters { _toFragment() }
          public var warmBloodedDetails: WarmBloodedDetails { _toFragment() }
          public var petDetails: PetDetails { _toFragment() }
        }

        /// AllAnimal.AsDog.Height
        public struct Height: TestSchema.SelectionSet {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Height }

          public var feet: Int { __data["feet"] }
          public var inches: Int? { __data["inches"] }
          public var meters: Int { __data["meters"] }
          public var relativeSize: GraphQLEnum<TestSchema.RelativeSize> { __data["relativeSize"] }
          public var centimeters: Double { __data["centimeters"] }
        }

        public typealias Owner = PetDetails.Owner
      }
    }
  }
}

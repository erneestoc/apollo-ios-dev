// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
import TestSchema

public class ClassroomPetsQuery: GraphQLQuery {
  public static let operationName: String = "ClassroomPets"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query ClassroomPets { classroomPets { __typename ...ClassroomPetDetails } }"#,
      fragments: [ClassroomPetDetails.self]
    ))

  public init() {}

  public struct Data: TestSchema.SelectionSet {
    public let __data: DataDict
    public init(_dataDict: DataDict) { __data = _dataDict }

    public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Query }
    public static var __selections: [ApolloAPI.Selection] { [
      .field("classroomPets", [ClassroomPet?]?.self),
    ] }

    public var classroomPets: [ClassroomPet?]? { __data["classroomPets"] }

    /// ClassroomPet
    public struct ClassroomPet: TestSchema.SelectionSet {
      public let __data: DataDict
      public init(_dataDict: DataDict) { __data = _dataDict }

      public static var __parentType: any ApolloAPI.ParentType { TestSchema.Unions.ClassroomPet }
      public static var __selections: [ApolloAPI.Selection] { [
        .field("__typename", String.self),
        .fragment(ClassroomPetDetails.self),
      ] }

      public var asRenamedAnimal: AsRenamedAnimal? { _asInlineFragment() }
      public var asPet: AsPet? { _asInlineFragment() }
      public var asWarmBlooded: AsWarmBlooded? { _asInlineFragment() }
      public var asCat: AsCat? { _asInlineFragment() }
      public var asBird: AsBird? { _asInlineFragment() }
      public var asPetRock: AsPetRock? { _asInlineFragment() }

      public struct Fragments: FragmentContainer {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public var classroomPetDetails: ClassroomPetDetails { _toFragment() }
      }

      /// ClassroomPet.AsRenamedAnimal
      public struct AsRenamedAnimal: TestSchema.InlineFragment, ApolloAPI.CompositeInlineFragment {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = ClassroomPetsQuery.Data.ClassroomPet
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.RenamedAnimal }
        public static var __mergedSources: [any ApolloAPI.SelectionSet.Type] { [
          ClassroomPetsQuery.Data.ClassroomPet.self,
          ClassroomPetDetails.AsRenamedAnimal.self
        ] }

        public var species: String { __data["species"] }

        public struct Fragments: FragmentContainer {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public var classroomPetDetails: ClassroomPetDetails { _toFragment() }
        }
      }

      /// ClassroomPet.AsPet
      public struct AsPet: TestSchema.InlineFragment, ApolloAPI.CompositeInlineFragment {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = ClassroomPetsQuery.Data.ClassroomPet
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.Pet }
        public static var __mergedSources: [any ApolloAPI.SelectionSet.Type] { [
          ClassroomPetsQuery.Data.ClassroomPet.self,
          ClassroomPetDetails.AsPet.self
        ] }

        public var humanName: String? { __data["humanName"] }

        public struct Fragments: FragmentContainer {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public var classroomPetDetails: ClassroomPetDetails { _toFragment() }
        }
      }

      /// ClassroomPet.AsWarmBlooded
      public struct AsWarmBlooded: TestSchema.InlineFragment, ApolloAPI.CompositeInlineFragment {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = ClassroomPetsQuery.Data.ClassroomPet
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Interfaces.WarmBlooded }
        public static var __mergedSources: [any ApolloAPI.SelectionSet.Type] { [
          ClassroomPetsQuery.Data.ClassroomPet.self,
          ClassroomPetDetails.AsRenamedAnimal.self,
          ClassroomPetDetails.AsWarmBlooded.self
        ] }

        public var species: String { __data["species"] }
        public var laysEggs: Bool { __data["laysEggs"] }

        public struct Fragments: FragmentContainer {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public var classroomPetDetails: ClassroomPetDetails { _toFragment() }
        }
      }

      /// ClassroomPet.AsCat
      public struct AsCat: TestSchema.InlineFragment, ApolloAPI.CompositeInlineFragment {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = ClassroomPetsQuery.Data.ClassroomPet
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Cat }
        public static var __mergedSources: [any ApolloAPI.SelectionSet.Type] { [
          ClassroomPetsQuery.Data.ClassroomPet.self,
          ClassroomPetDetails.AsRenamedAnimal.self,
          ClassroomPetDetails.AsPet.self,
          ClassroomPetDetails.AsWarmBlooded.self,
          ClassroomPetDetails.AsCat.self
        ] }

        public var species: String { __data["species"] }
        public var humanName: String? { __data["humanName"] }
        public var laysEggs: Bool { __data["laysEggs"] }
        public var bodyTemperature: Int { __data["bodyTemperature"] }
        public var isJellicle: Bool { __data["isJellicle"] }

        public struct Fragments: FragmentContainer {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public var classroomPetDetails: ClassroomPetDetails { _toFragment() }
        }
      }

      /// ClassroomPet.AsBird
      public struct AsBird: TestSchema.InlineFragment, ApolloAPI.CompositeInlineFragment {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = ClassroomPetsQuery.Data.ClassroomPet
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Bird }
        public static var __mergedSources: [any ApolloAPI.SelectionSet.Type] { [
          ClassroomPetsQuery.Data.ClassroomPet.self,
          ClassroomPetDetails.AsRenamedAnimal.self,
          ClassroomPetDetails.AsPet.self,
          ClassroomPetDetails.AsWarmBlooded.self,
          ClassroomPetDetails.AsBird.self
        ] }

        public var species: String { __data["species"] }
        public var humanName: String? { __data["humanName"] }
        public var laysEggs: Bool { __data["laysEggs"] }
        public var wingspan: Double { __data["wingspan"] }

        public struct Fragments: FragmentContainer {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public var classroomPetDetails: ClassroomPetDetails { _toFragment() }
        }
      }

      /// ClassroomPet.AsPetRock
      public struct AsPetRock: TestSchema.InlineFragment, ApolloAPI.CompositeInlineFragment {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = ClassroomPetsQuery.Data.ClassroomPet
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.PetRock }
        public static var __mergedSources: [any ApolloAPI.SelectionSet.Type] { [
          ClassroomPetsQuery.Data.ClassroomPet.self,
          ClassroomPetDetails.AsPet.self,
          ClassroomPetDetails.AsPetRock.self
        ] }

        public var humanName: String? { __data["humanName"] }
        public var favoriteToy: String { __data["favoriteToy"] }

        public struct Fragments: FragmentContainer {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public var classroomPetDetails: ClassroomPetDetails { _toFragment() }
        }
      }
    }
  }
}

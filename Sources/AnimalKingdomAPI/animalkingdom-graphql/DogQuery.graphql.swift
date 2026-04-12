// @generated
// This file was automatically generated and should not be edited.

@_exported import ApolloAPI
import TestSchema

public class DogQuery: GraphQLQuery {
  public static let operationName: String = "DogQuery"
  public static let operationDocument: ApolloAPI.OperationDocument = .init(
    definition: .init(
      #"query DogQuery { allAnimals { __typename id skinCovering ... on Dog { ...DogFragment houseDetails } } }"#,
      fragments: [DogFragment.self]
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
        .field("skinCovering", GraphQLEnum<TestSchema.SkinCovering>?.self),
        .inlineFragment(AsDog.self),
      ] }

      public var id: TestSchema.ID { __data["id"] }
      public var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }

      public var asDog: AsDog? { _asInlineFragment() }

      /// AllAnimal.AsDog
      public struct AsDog: TestSchema.InlineFragment {
        public let __data: DataDict
        public init(_dataDict: DataDict) { __data = _dataDict }

        public typealias RootEntityType = DogQuery.Data.AllAnimal
        public static var __parentType: any ApolloAPI.ParentType { TestSchema.Objects.Dog }
        public static var __selections: [ApolloAPI.Selection] { [
          .field("houseDetails", TestSchema.Object?.self),
          .fragment(DogFragment.self),
        ] }

        public var houseDetails: TestSchema.Object? { __data["houseDetails"] }
        public var id: TestSchema.ID { __data["id"] }
        public var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        public var species: String { __data["species"] }

        public struct Fragments: FragmentContainer {
          public let __data: DataDict
          public init(_dataDict: DataDict) { __data = _dataDict }

          public var dogFragment: DogFragment { _toFragment() }
        }
      }
    }
  }
}

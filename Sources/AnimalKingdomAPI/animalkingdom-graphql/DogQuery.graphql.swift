// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

class DogQuery: GraphQLQuery {
  static let operationName: String = "DogQuery"
  static let operationDocument: Apollo.OperationDocument = .init(
    operationIdentifier: "b4567865a9c655575b64f02654cb844a1f719d1097e64a7d3ca949699671929e"
  )

  init() {}

  struct Data: TestSchema.SelectionSet {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Query }
    static var __selections: [Apollo.Selection] { [
      .field("allAnimals", [AllAnimal].self),
    ] }

    var allAnimals: [AllAnimal] { __data["allAnimals"] }

    /// AllAnimal
    ///
    /// Parent Type: `RenamedAnimal`
    struct AllAnimal: TestSchema.SelectionSet {
      let __data: DataDict
      init(_dataDict: DataDict) { __data = _dataDict }

      static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.RenamedAnimal }
      static var __selections: [Apollo.Selection] { [
        .field("__typename", String.self),
        .field("id", TestSchema.ID.self),
        .field("skinCovering", GraphQLEnum<TestSchema.SkinCovering>?.self),
        .inlineFragment(AsDog.self),
      ] }

      var id: TestSchema.ID { __data["id"] }
      var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }

      var asDog: AsDog? { _asInlineFragment() }

      /// AllAnimal.AsDog
      ///
      /// Parent Type: `Dog`
      struct AsDog: TestSchema.InlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = DogQuery.Data.AllAnimal
        static var __parentType: any Apollo.ParentType { TestSchema.Objects.Dog }
        static var __selections: [Apollo.Selection] { [
          .field("houseDetails", TestSchema.Object?.self),
          .fragment(DogFragment.self),
        ] }

        var houseDetails: TestSchema.Object? { __data["houseDetails"] }
        var id: TestSchema.ID { __data["id"] }
        var skinCovering: GraphQLEnum<TestSchema.SkinCovering>? { __data["skinCovering"] }
        var species: String { __data["species"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var dogFragment: DogFragment { _toFragment() }
        }
      }
    }
  }
}

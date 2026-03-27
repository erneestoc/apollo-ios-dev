// @generated
// This file was automatically generated and should not be edited.

@_exported import Apollo
import App

class ClassroomPetsQuery: GraphQLQuery {
  static let operationName: String = "ClassroomPets"
  static let operationDocument: Apollo.OperationDocument = .init(
    operationIdentifier: "3b9964970aa7888c07400f542d82403d5a302d1d6707e06eadae1d643610022e"
  )

  init() {}

  struct Data: TestSchema.SelectionSet {
    let __data: DataDict
    init(_dataDict: DataDict) { __data = _dataDict }

    static var __parentType: any Apollo.ParentType { TestSchema.Objects.Query }
    static var __selections: [Apollo.Selection] { [
      .field("classroomPets", [ClassroomPet?]?.self),
    ] }

    var classroomPets: [ClassroomPet?]? { __data["classroomPets"] }

    /// ClassroomPet
    ///
    /// Parent Type: `ClassroomPet`
    struct ClassroomPet: TestSchema.SelectionSet {
      let __data: DataDict
      init(_dataDict: DataDict) { __data = _dataDict }

      static var __parentType: any Apollo.ParentType { TestSchema.Unions.ClassroomPet }
      static var __selections: [Apollo.Selection] { [
        .field("__typename", String.self),
        .fragment(ClassroomPetDetails.self),
      ] }

      var asRenamedAnimal: AsRenamedAnimal? { _asInlineFragment() }
      var asPet: AsPet? { _asInlineFragment() }
      var asWarmBlooded: AsWarmBlooded? { _asInlineFragment() }
      var asCat: AsCat? { _asInlineFragment() }
      var asBird: AsBird? { _asInlineFragment() }
      var asPetRock: AsPetRock? { _asInlineFragment() }

      struct Fragments: FragmentContainer {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        var classroomPetDetails: ClassroomPetDetails { _toFragment() }
      }

      /// ClassroomPet.AsRenamedAnimal
      ///
      /// Parent Type: `RenamedAnimal`
      struct AsRenamedAnimal: TestSchema.InlineFragment, Apollo.CompositeInlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = ClassroomPetsQuery.Data.ClassroomPet
        static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.RenamedAnimal }
        static var __mergedSources: [any Apollo.SelectionSet.Type] { [
          ClassroomPetsQuery.Data.ClassroomPet.self,
          ClassroomPetDetails.AsRenamedAnimal.self
        ] }

        var species: String { __data["species"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var classroomPetDetails: ClassroomPetDetails { _toFragment() }
        }
      }

      /// ClassroomPet.AsPet
      ///
      /// Parent Type: `Pet`
      struct AsPet: TestSchema.InlineFragment, Apollo.CompositeInlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = ClassroomPetsQuery.Data.ClassroomPet
        static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.Pet }
        static var __mergedSources: [any Apollo.SelectionSet.Type] { [
          ClassroomPetsQuery.Data.ClassroomPet.self,
          ClassroomPetDetails.AsPet.self
        ] }

        var humanName: String? { __data["humanName"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var classroomPetDetails: ClassroomPetDetails { _toFragment() }
        }
      }

      /// ClassroomPet.AsWarmBlooded
      ///
      /// Parent Type: `WarmBlooded`
      struct AsWarmBlooded: TestSchema.InlineFragment, Apollo.CompositeInlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = ClassroomPetsQuery.Data.ClassroomPet
        static var __parentType: any Apollo.ParentType { TestSchema.Interfaces.WarmBlooded }
        static var __mergedSources: [any Apollo.SelectionSet.Type] { [
          ClassroomPetsQuery.Data.ClassroomPet.self,
          ClassroomPetDetails.AsRenamedAnimal.self,
          ClassroomPetDetails.AsWarmBlooded.self
        ] }

        var species: String { __data["species"] }
        var laysEggs: Bool { __data["laysEggs"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var classroomPetDetails: ClassroomPetDetails { _toFragment() }
        }
      }

      /// ClassroomPet.AsCat
      ///
      /// Parent Type: `Cat`
      struct AsCat: TestSchema.InlineFragment, Apollo.CompositeInlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = ClassroomPetsQuery.Data.ClassroomPet
        static var __parentType: any Apollo.ParentType { TestSchema.Objects.Cat }
        static var __mergedSources: [any Apollo.SelectionSet.Type] { [
          ClassroomPetsQuery.Data.ClassroomPet.self,
          ClassroomPetDetails.AsRenamedAnimal.self,
          ClassroomPetDetails.AsPet.self,
          ClassroomPetDetails.AsWarmBlooded.self,
          ClassroomPetDetails.AsCat.self
        ] }

        var species: String { __data["species"] }
        var humanName: String? { __data["humanName"] }
        var laysEggs: Bool { __data["laysEggs"] }
        var bodyTemperature: Int { __data["bodyTemperature"] }
        var isJellicle: Bool { __data["isJellicle"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var classroomPetDetails: ClassroomPetDetails { _toFragment() }
        }
      }

      /// ClassroomPet.AsBird
      ///
      /// Parent Type: `Bird`
      struct AsBird: TestSchema.InlineFragment, Apollo.CompositeInlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = ClassroomPetsQuery.Data.ClassroomPet
        static var __parentType: any Apollo.ParentType { TestSchema.Objects.Bird }
        static var __mergedSources: [any Apollo.SelectionSet.Type] { [
          ClassroomPetsQuery.Data.ClassroomPet.self,
          ClassroomPetDetails.AsRenamedAnimal.self,
          ClassroomPetDetails.AsPet.self,
          ClassroomPetDetails.AsWarmBlooded.self,
          ClassroomPetDetails.AsBird.self
        ] }

        var species: String { __data["species"] }
        var humanName: String? { __data["humanName"] }
        var laysEggs: Bool { __data["laysEggs"] }
        var wingspan: Double { __data["wingspan"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var classroomPetDetails: ClassroomPetDetails { _toFragment() }
        }
      }

      /// ClassroomPet.AsPetRock
      ///
      /// Parent Type: `PetRock`
      struct AsPetRock: TestSchema.InlineFragment, Apollo.CompositeInlineFragment {
        let __data: DataDict
        init(_dataDict: DataDict) { __data = _dataDict }

        typealias RootEntityType = ClassroomPetsQuery.Data.ClassroomPet
        static var __parentType: any Apollo.ParentType { TestSchema.Objects.PetRock }
        static var __mergedSources: [any Apollo.SelectionSet.Type] { [
          ClassroomPetsQuery.Data.ClassroomPet.self,
          ClassroomPetDetails.AsPet.self,
          ClassroomPetDetails.AsPetRock.self
        ] }

        var humanName: String? { __data["humanName"] }
        var favoriteToy: String { __data["favoriteToy"] }

        struct Fragments: FragmentContainer {
          let __data: DataDict
          init(_dataDict: DataDict) { __data = _dataDict }

          var classroomPetDetails: ClassroomPetDetails { _toFragment() }
        }
      }
    }
  }
}
